//! The edit server, a Unix socket that `aiditor --wait` clients in terminals send files to, and
//! that client.

use std::collections::HashMap;
use std::ffi::OsString;
use std::fs::{self, Permissions};
use std::io::{self, BufRead, Write};
use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use aiditor_core::{ClientFile, Msg, TerminalId};
use serde::Deserialize;
use serde_json::json;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::{mpsc::Sender, oneshot};

/// The longest request line, so a client can't make the editor buffer without bound.
const LIMIT: u64 = 1 << 20;

type Reply = Result<(), String>;
type Pending = Arc<Mutex<HashMap<u64, oneshot::Sender<Reply>>>>;

/// `aiditor/<pid>.sock` in the per-user runtime dir; `None` on Linux without `XDG_RUNTIME_DIR`,
/// since a shared `/tmp/aiditor` could be planted by another user.
pub fn socket_path() -> Option<PathBuf> {
    let dir = if cfg!(target_os = "macos") {
        Some(std::env::temp_dir())
    } else {
        std::env::var_os("XDG_RUNTIME_DIR").map(PathBuf::from)
    };
    let dir = dir.filter(|dir| dir.is_absolute())?;
    Some(dir.join(format!("aiditor/{}.sock", std::process::id())))
}

/// A listening socket; dropping it stops listening and removes the socket file.
pub struct Server {
    path: PathBuf,
    pending: Pending,
    task: tokio::task::JoinHandle<()>,
}

impl Server {
    /// Listens at `path`, 0600 in a 0700 directory, replacing a stale socket; each request goes
    /// to the core on `tx` as `Msg::EditRequest`.
    pub fn listen(path: PathBuf, tx: Sender<Msg>) -> io::Result<Server> {
        let dir = path.parent().expect("the socket is in a directory");
        fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(dir)?;
        fs::set_permissions(dir, Permissions::from_mode(0o700))?;
        match fs::remove_file(&path) {
            Err(err) if err.kind() != io::ErrorKind::NotFound => return Err(err),
            _ => {}
        }
        let listener = UnixListener::bind(&path)?;
        fs::set_permissions(&path, Permissions::from_mode(0o600))?;
        let pending = Pending::default();
        let task = tokio::spawn(accept(listener, tx, pending.clone()));
        Ok(Server {
            path,
            pending,
            task,
        })
    }

    /// The variables a terminal needs to run the client, all but its `AIDITOR_TERMINAL`.
    pub fn env(&self) -> Vec<(String, String)> {
        let exe = match std::env::current_exe() {
            Ok(exe) => exe,
            Err(err) => {
                tracing::warn!("no $EDITOR client in terminals: {err}");
                return Vec::new();
            }
        };
        let command = format!("{} --wait", quote(&exe.to_string_lossy()));
        let mut env = vec![(
            "AIDITOR_SOCKET".to_string(),
            self.path.to_string_lossy().into_owned(),
        )];
        for key in ["GIT_EDITOR", "EDITOR", "VISUAL"] {
            env.push((key.into(), command.clone()));
        }
        env
    }

    /// Answers request `request`; ignored when its client has gone.
    pub fn reply(&self, request: u64, result: Reply) {
        if let Some(client) = self.pending.lock().unwrap().remove(&request) {
            _ = client.send(result);
        }
    }
}

/// `path` quoted for sh, as git runs `$GIT_EDITOR` through `sh -c`; bare when it is safe, as
/// git quotes the command again in its errors.
fn quote(path: &str) -> String {
    let safe = |c: char| c.is_ascii_alphanumeric() || "/._-+".contains(c);
    match !path.is_empty() && path.chars().all(safe) {
        true => path.into(),
        false => format!("'{}'", path.replace('\'', r"'\''")),
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        self.task.abort();
        // Waiting clients see their connection close and exit 1.
        self.pending.lock().unwrap().clear();
        if let Err(err) = fs::remove_file(&self.path) {
            tracing::warn!("removing {}: {err}", self.path.display());
        }
    }
}

async fn accept(listener: UnixListener, tx: Sender<Msg>, pending: Pending) {
    let mut request = 0;
    loop {
        match listener.accept().await {
            Ok((stream, _)) => {
                request += 1;
                tokio::spawn(serve(stream, request, tx.clone(), pending.clone()));
            }
            Err(err) => {
                tracing::warn!("edit server: {err}");
                // Out of file descriptors, say; waits rather than spinning.
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        }
    }
}

/// Reads one request line, hands it to the core and writes back the core's reply. A client that
/// goes away first is forgotten, and the core told.
/// shortcut: no read timeout; add one if idle connections pile up.
async fn serve(stream: UnixStream, request: u64, tx: Sender<Msg>, pending: Pending) {
    let (read, mut write) = stream.into_split();
    let mut read = BufReader::new(read.take(LIMIT));
    let mut line = String::new();
    let parsed = match read.read_line(&mut line).await {
        Err(err) => Err(format!("Bad request: {err}")),
        Ok(_) if !line.ends_with('\n') => Err("Bad request: too long or cut off".into()),
        Ok(_) => parse(&line),
    };
    let result = match parsed {
        Err(err) => Err(err),
        Ok((terminal, files)) => {
            let (client, reply) = oneshot::channel();
            pending.lock().unwrap().insert(request, client);
            let msg = Msg::EditRequest {
                request,
                terminal,
                files,
            };
            if tx.send(msg).await.is_err() {
                pending.lock().unwrap().remove(&request);
                return;
            }
            let mut byte = [0];
            tokio::select! {
                reply = reply => match reply {
                    Ok(result) => result,
                    Err(_) => return,
                },
                // The client only waits, so anything readable means it hung up.
                _ = read.get_mut().get_mut().read(&mut byte) => {
                    pending.lock().unwrap().remove(&request);
                    _ = tx.send(Msg::EditGone { request }).await;
                    return;
                }
            }
        }
    };
    let reply = match result {
        Ok(()) => json!({ "code": 0 }),
        Err(error) => json!({ "code": 1, "error": error }),
    };
    _ = write.write_all(format!("{reply}\n").as_bytes()).await;
}

#[derive(Deserialize)]
struct Request {
    cwd: PathBuf,
    terminal: u64,
    args: Vec<String>,
}

/// A request line's terminal and files: absolute, each at the `+LINE[:COL]` just before it.
fn parse(line: &str) -> Result<(TerminalId, Vec<ClientFile>), String> {
    let request: Request =
        serde_json::from_str(line).map_err(|err| format!("Bad request: {err}"))?;
    if !request.cwd.is_absolute() {
        return Err("Bad request: the working directory is relative".into());
    }
    let mut files = Vec::new();
    let mut at = None;
    for arg in request.args {
        if let Some(position) = position(&arg) {
            at = Some(position);
        } else if arg.is_empty() {
            return Err("Bad request: an empty file name".into());
        } else {
            let (line, col) = at.take().unzip();
            files.push(ClientFile {
                path: request.cwd.join(arg),
                line,
                col: col.flatten(),
            });
        }
    }
    if files.is_empty() {
        return Err("No file to edit".into());
    }
    Ok((TerminalId(request.terminal), files))
}

/// `+LINE` or `+LINE:COL`.
fn position(arg: &str) -> Option<(usize, Option<usize>)> {
    let arg = arg.strip_prefix('+')?;
    let (line, col) = match arg.split_once(':') {
        Some((line, col)) => (line, Some(col.parse().ok()?)),
        None => (arg, None),
    };
    Some((line.parse().ok()?, col))
}

/// `aiditor --wait ARGS`: sends ARGS to the editor at `socket` for terminal `terminal`, blocks
/// for its reply and returns the exit code, printing why on stderr.
pub fn client(socket: Option<OsString>, terminal: Option<String>, args: Vec<String>) -> i32 {
    let (code, error) =
        wait(socket, terminal, args).unwrap_or_else(|(code, err)| (code, Some(err)));
    if let Some(error) = error {
        eprintln!("aiditor: {error}");
    }
    code
}

fn wait(
    socket: Option<OsString>,
    terminal: Option<String>,
    args: Vec<String>,
) -> Result<(i32, Option<String>), (i32, String)> {
    let socket = (socket.filter(|s| !s.is_empty())).ok_or((
        2,
        "--wait runs in an aiditor terminal, and AIDITOR_SOCKET is not set".into(),
    ))?;
    let terminal: u64 = (terminal.and_then(|t| t.parse().ok()))
        .ok_or((2, "AIDITOR_TERMINAL is not a terminal id".to_string()))?;
    let cwd = std::env::current_dir().map_err(|err| (2, format!("working directory: {err}")))?;
    let cwd = cwd
        .to_str()
        .ok_or((2, "the working directory is not UTF-8".into()))?;
    let socket = Path::new(&socket);
    let mut stream = std::os::unix::net::UnixStream::connect(socket)
        .map_err(|err| (2, format!("{}: {err}", socket.display())))?;
    let lost = || (1, "lost the connection to the editor".to_string());
    let request = json!({ "cwd": cwd, "terminal": terminal, "args": args });
    writeln!(stream, "{request}").map_err(|_| lost())?;
    let mut line = String::new();
    io::BufReader::new(stream)
        .read_line(&mut line)
        .map_err(|_| lost())?;
    #[derive(Deserialize)]
    struct Answer {
        code: i32,
        error: Option<String>,
    }
    // The connection closing first leaves the line empty, which doesn't parse.
    let answer: Answer = serde_json::from_str(&line).map_err(|_| lost())?;
    Ok((answer.code, answer.error))
}

#[cfg(test)]
mod tests {
    use std::os::unix::fs::MetadataExt;

    use super::*;

    fn server(dir: &Path) -> (Server, tokio::sync::mpsc::Receiver<Msg>) {
        let (tx, rx) = crate::channel();
        (Server::listen(dir.join("aiditor/1.sock"), tx).unwrap(), rx)
    }

    /// Runs the client off the runtime, for terminal 7.
    fn spawn_client(server: &Server, args: &[&str]) -> tokio::task::JoinHandle<i32> {
        let socket = Some(server.path.clone().into());
        let args = args.iter().map(|a| a.to_string()).collect();
        tokio::task::spawn_blocking(move || client(socket, Some("7".into()), args))
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn a_request_reaches_the_core_and_the_client_exits_with_the_reply() {
        let dir = tempfile::tempdir().unwrap();
        let (server, mut rx) = server(dir.path());
        let cwd = std::env::current_dir().unwrap();
        for (result, code) in [(Ok(()), 0), (Err("no".to_string()), 1)] {
            let exit = spawn_client(&server, &["+3:5", "a.txt", "+2", "/b", "c"]);
            let Some(Msg::EditRequest {
                request,
                terminal,
                files,
            }) = rx.recv().await
            else {
                panic!("no request");
            };
            assert_eq!(terminal, TerminalId(7));
            let file = |path: PathBuf, line, col| ClientFile { path, line, col };
            assert_eq!(
                files,
                [
                    file(cwd.join("a.txt"), Some(3), Some(5)),
                    file("/b".into(), Some(2), None),
                    file(cwd.join("c"), None, None),
                ]
            );
            server.reply(request, result);
            assert_eq!(exit.await.unwrap(), code);
        }
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn bad_requests_are_refused_and_the_server_keeps_serving() {
        let dir = tempfile::tempdir().unwrap();
        let (server, mut rx) = server(dir.path());
        let refused = |line: &'static str| {
            let path = server.path.clone();
            tokio::task::spawn_blocking(move || {
                let mut stream = std::os::unix::net::UnixStream::connect(path).unwrap();
                stream.write_all(line.as_bytes()).unwrap();
                let mut reply = String::new();
                io::BufReader::new(stream).read_line(&mut reply).unwrap();
                reply
            })
        };
        for line in [
            "not json\n",
            "{\"cwd\":\"w\",\"terminal\":1,\"args\":[\"a\"]}\n",
            "{\"cwd\":\"/w\",\"terminal\":1,\"args\":[\"+1\"]}\n",
        ] {
            let reply = refused(line).await.unwrap();
            assert!(reply.starts_with("{\"code\":1,\"error\":"), "{reply}");
        }
        assert!(rx.try_recv().is_err(), "the core heard nothing");
        let exit = spawn_client(&server, &["a"]);
        let Some(Msg::EditRequest { request, .. }) = rx.recv().await else {
            panic!("no request");
        };
        server.reply(request, Ok(()));
        assert_eq!(exit.await.unwrap(), 0);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn a_client_that_hangs_up_is_reported() {
        let dir = tempfile::tempdir().unwrap();
        let (server, mut rx) = server(dir.path());
        let mut stream = UnixStream::connect(&server.path).await.unwrap();
        let line = "{\"cwd\":\"/w\",\"terminal\":1,\"args\":[\"a\"]}\n";
        stream.write_all(line.as_bytes()).await.unwrap();
        let Some(Msg::EditRequest { request, .. }) = rx.recv().await else {
            panic!("no request");
        };
        drop(stream);
        assert!(matches!(rx.recv().await, Some(Msg::EditGone { request: r }) if r == request));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn waiting_clients_exit_1_when_the_server_goes() {
        let dir = tempfile::tempdir().unwrap();
        let (server, mut rx) = server(dir.path());
        let exit = spawn_client(&server, &["a"]);
        assert!(matches!(rx.recv().await, Some(Msg::EditRequest { .. })));
        drop(server);
        assert_eq!(exit.await.unwrap(), 1);
    }

    #[tokio::test]
    async fn the_socket_replaces_a_stale_one_is_private_and_gone_after_drop() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir(dir.path().join("aiditor")).unwrap();
        fs::write(dir.path().join("aiditor/1.sock"), "stale").unwrap();
        let (server, _rx) = server(dir.path());
        let path = server.path.clone();
        assert_eq!(fs::metadata(&path).unwrap().mode() & 0o777, 0o600);
        assert_eq!(
            fs::metadata(path.parent().unwrap()).unwrap().mode() & 0o777,
            0o700
        );
        drop(server);
        assert!(!path.exists());
    }

    #[test]
    fn the_client_without_an_editor_fails() {
        let dir = tempfile::tempdir().unwrap();
        let terminal = || Some("1".to_string());
        let args = || vec!["a".to_string()];
        assert_eq!(client(None, terminal(), args()), 2);
        let dead = dir.path().join("dead.sock");
        assert_eq!(client(Some(dead.clone().into()), terminal(), args()), 2);
        assert_eq!(client(Some(dead.clone().into()), None, args()), 2);
        // An editor that hangs up before replying.
        let listener = std::os::unix::net::UnixListener::bind(&dead).unwrap();
        let hang_up = std::thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            let mut line = String::new();
            io::BufReader::new(stream).read_line(&mut line).unwrap();
        });
        assert_eq!(client(Some(dead.into()), terminal(), args()), 1);
        hang_up.join().unwrap();
    }

    #[test]
    fn paths_are_quoted_only_when_they_need_it() {
        assert_eq!(quote("/a/b-1/aiditor"), "/a/b-1/aiditor");
        assert_eq!(quote("/My Apps/it's"), r"'/My Apps/it'\''s'");
        assert_eq!(quote(""), "''");
    }

    #[test]
    fn the_env_runs_the_client_quoted() {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let _guard = runtime.enter();
        let dir = tempfile::tempdir().unwrap();
        let (server, _rx) = server(dir.path());
        let env: HashMap<_, _> = server.env().into_iter().collect();
        let exe = std::env::current_exe().unwrap();
        let command = format!("{} --wait", quote(&exe.to_string_lossy()));
        assert_eq!(env["EDITOR"], command);
        assert_eq!(env["GIT_EDITOR"], command);
        assert_eq!(env["VISUAL"], command);
        assert_eq!(env["AIDITOR_SOCKET"], server.path.display().to_string());
    }
}
