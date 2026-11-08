//! ACP agent processes, each speaking JSON-RPC over its stdio and sending on its own bounded
//! channel.

use std::collections::HashMap;
use std::ffi::CStr;
use std::os::unix::process::ExitStatusExt;
use std::path::{Path, PathBuf};
use std::process::{ExitStatus, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use agent_client_protocol::schema::ProtocolVersion;
use agent_client_protocol::schema::v1::{
    AgentRequest, CancelNotification, ClientCapabilities, ContentBlock, DeleteSessionRequest,
    ElicitationCapabilities, ElicitationFormCapabilities, ForkSessionRequest, Implementation,
    InitializeRequest, ListSessionsRequest, LoadSessionRequest, NewSessionRequest,
    NewSessionResponse, PromptRequest, RequestId, SessionInfo, SessionNotification,
};
use agent_client_protocol::{Agent, ByteStreams, Client, ConnectionTo, Responder};
use aiditor_core::{AgentId, AgentOp, Msg, Open, Request};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};
use tokio::sync::mpsc::{Receiver, Sender, channel};
use tokio::sync::oneshot;
use tokio::time::timeout;
use tokio_util::compat::{TokioAsyncReadCompatExt, TokioAsyncWriteCompatExt};

/// The agent's requests the core hasn't answered yet, each with a sender whose drop ends the
/// wait for its withdrawal; ACP's request enum takes its answer as JSON, so replies are
/// serialized.
type Pending = Arc<Mutex<HashMap<RequestId, (Responder, oneshot::Sender<()>)>>>;

/// The spawned agents by id, and the processes still listing sessions by request.
#[derive(Default)]
pub struct Agents {
    handles: HashMap<AgentId, Handle>,
    lists: Lists,
}

/// Each running list's process group and whether it exited; its task removes it to terminate it.
type Lists = Arc<Mutex<HashMap<u64, (libc::pid_t, Arc<AtomicBool>)>>>;

/// The connection to the agent and its sender, while its session is open; dropped with the
/// connection so the receiver closes after the exit.
type Connection = Arc<Mutex<Option<(ConnectionTo<Agent>, Sender<Msg>)>>>;

struct Handle {
    /// The agent leads its own process group, so this is the group's id too.
    pid: libc::pid_t,
    exited: Arc<AtomicBool>,
    /// Whether it advertised `session/delete`, once initialized.
    deletable: Arc<AtomicBool>,
    pending: Pending,
    connection: Connection,
}

/// How long a terminated agent has to exit before SIGKILL.
const GRACE: Duration = Duration::from_millis(100);

/// How long an agent has to answer `session/delete` before it is stopped all the same.
const DELETE_WAIT: Duration = Duration::from_secs(5);

/// shortcut: sessions past this many `session/list` pages are left out; page on demand if a
/// real history ever gets that long.
const LIST_PAGES: usize = 50;

impl Handle {
    /// SIGTERM to the agent's group unless it has exited; returns the group for the SIGKILL.
    fn terminate(self) -> Option<libc::pid_t> {
        self.pending.lock().unwrap().clear();
        terminate(self.pid, &self.exited)
    }
}

/// Sends `request` as `session/prompt`, its answer as `Msg::AgentPrompted`.
fn prompt(connection: &ConnectionTo<Agent>, tx: Sender<Msg>, id: AgentId, request: PromptRequest) {
    // The answer is handled before the next message, so it follows the turn's updates.
    let sent = connection
        .send_request(request)
        .on_receiving_result(move |result| async move {
            let result = result.map(|r| r.stop_reason).map_err(|err| err.message);
            _ = tx.send(Msg::AgentPrompted { id, result }).await;
            Ok(())
        });
    if let Err(err) = sent {
        tracing::warn!("prompting the agent: {err}");
    }
}

/// `request` with each image block that has no data filled from the file its URI names.
async fn read_images(mut request: PromptRequest) -> Result<PromptRequest, String> {
    for block in &mut request.prompt {
        if let ContentBlock::Image(image) = block
            && image.data.is_empty()
        {
            let uri = image.uri.as_deref().unwrap_or_default();
            let path = uri.strip_prefix("file://").unwrap_or(uri);
            let bytes =
                (tokio::fs::read(path).await).map_err(|err| format!("can't read {path}: {err}"))?;
            image.data = base64(&bytes);
        }
    }
    Ok(request)
}

/// `bytes` in standard, padded base64.
fn base64(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let n = chunk
            .iter()
            .enumerate()
            .fold(0u32, |n, (i, &b)| n | u32::from(b) << (16 - 8 * i));
        for i in 0..4 {
            out.push(match i <= chunk.len() {
                true => ALPHABET[(n >> (18 - 6 * i) & 63) as usize] as char,
                false => '=',
            });
        }
    }
    out
}

/// SIGTERM to group `pid` unless the agent has exited; returns the group for the SIGKILL.
fn terminate(pid: libc::pid_t, exited: &AtomicBool) -> Option<libc::pid_t> {
    // shortcut: the group can empty between this check and the SIGKILL, and its id be reused
    // within the grace, as with terminals; track every member if that ever shows up.
    if exited.load(Ordering::SeqCst) {
        return None;
    }
    unsafe { libc::killpg(pid, libc::SIGTERM) };
    Some(pid)
}

/// SIGKILL to `group` after the grace, without blocking.
fn kill_after_grace(group: libc::pid_t) {
    thread::spawn(move || {
        thread::sleep(GRACE);
        unsafe { libc::killpg(group, libc::SIGKILL) };
    });
}

impl Agents {
    /// Run `op` without blocking, on the Tokio runtime; a spawn returns its agent's receiver.
    pub fn run(&mut self, op: AgentOp) -> Option<Receiver<Msg>> {
        match op {
            AgentOp::Spawn {
                id,
                command,
                args,
                cwd,
                roots,
                env,
                session,
            } => Some(self.spawn(id, command, args, (cwd, roots, session), env)),
            AgentOp::Reply {
                id,
                request,
                result,
            } => {
                let responder = (self.handles.get(&id))
                    .and_then(|agent| agent.pending.lock().unwrap().remove(&request))
                    .map(|(responder, _)| responder);
                let result = result.map(|response| {
                    serde_json::to_value(response).expect("a ClientResponse serializes")
                });
                if let Some(responder) = responder
                    && let Err(err) = responder.respond_with_result(result)
                {
                    tracing::warn!("replying to the agent: {err}");
                }
                None
            }
            AgentOp::Prompt { id, request } => {
                let (connection, tx) = self.connection(id)?;
                let unread =
                    |b: &ContentBlock| matches!(b, ContentBlock::Image(i) if i.data.is_empty());
                if !request.prompt.iter().any(unread) {
                    prompt(&connection, tx, id, request);
                    return None;
                }
                // Reading images waits off the caller's thread; nothing can overtake it, as the
                // core sends no prompt while one runs.
                tokio::spawn(async move {
                    match read_images(request).await {
                        Ok(request) => prompt(&connection, tx, id, request),
                        Err(err) => {
                            _ = tx
                                .send(Msg::AgentPrompted {
                                    id,
                                    result: Err(err),
                                })
                                .await
                        }
                    }
                });
                None
            }
            AgentOp::SetConfigOption { id, request } => {
                let (connection, tx) = self.connection(id)?;
                let sent = connection.send_request(request).on_receiving_result(
                    move |result| async move {
                        let result = result.map(|r| r.config_options).map_err(|err| err.message);
                        _ = tx.send(Msg::AgentConfigured { id, result }).await;
                        Ok(())
                    },
                );
                if let Err(err) = sent {
                    tracing::warn!("setting the agent's config option: {err}");
                }
                None
            }
            AgentOp::Cancel { id, session } => {
                let sent = (self.connection(id)).map(|(connection, _)| {
                    connection.send_notification(CancelNotification::new(session))
                });
                if let Some(Err(err)) = sent {
                    tracing::warn!("cancelling the agent's turn: {err}");
                }
                None
            }
            AgentOp::Kill { id } => {
                if let Some(group) = self.handles.remove(&id).and_then(Handle::terminate) {
                    kill_after_grace(group);
                }
                None
            }
            AgentOp::Delete { id, session } => {
                let handle = self.handles.get(&id)?;
                let connection = (handle.connection.lock().unwrap().clone())
                    .filter(|_| handle.deletable.load(Ordering::SeqCst));
                let Some((connection, _)) = connection else {
                    return self.run(AgentOp::Kill { id });
                };
                // The handle stays until quitting, so quitting while the agent deletes still
                // kills it.
                let (pid, exited) = (handle.pid, handle.exited.clone());
                tokio::spawn(async move {
                    let delete = connection.send_request(DeleteSessionRequest::new(session));
                    match timeout(DELETE_WAIT, delete.block_task()).await {
                        Ok(Ok(_)) => {}
                        Ok(Err(err)) => tracing::warn!("deleting the session: {err}"),
                        Err(_) => tracing::warn!("deleting the session: no answer"),
                    }
                    if let Some(group) = terminate(pid, &exited) {
                        kill_after_grace(group);
                    }
                });
                None
            }
            AgentOp::List {
                request,
                command,
                args,
                cwd,
                env,
            } => Some(self.list(request, command, args, cwd, env)),
        }
    }

    /// The connection of agent `id` and its sender, once its session is open.
    fn connection(&self, id: AgentId) -> Option<(ConnectionTo<Agent>, Sender<Msg>)> {
        self.handles.get(&id)?.connection.lock().unwrap().clone()
    }

    /// Kill every agent, blocking for one shared grace, so no agent outlives the app.
    pub fn kill_all(&mut self) {
        let lists: Vec<_> = self.lists.lock().unwrap().drain().collect();
        let groups: Vec<_> = (self.handles.drain())
            .filter_map(|(_, agent)| agent.terminate())
            .chain(
                lists
                    .iter()
                    .filter_map(|(_, (pid, exited))| terminate(*pid, exited)),
            )
            .collect();
        if groups.is_empty() {
            return;
        }
        thread::sleep(GRACE);
        for group in groups {
            unsafe { libc::killpg(group, libc::SIGKILL) };
        }
    }

    /// Starts the process here, so a kill right after has a pid; the rest runs in its tasks.
    fn spawn(
        &mut self,
        id: AgentId,
        command: String,
        args: Vec<String>,
        (cwd, roots, session): (PathBuf, Vec<PathBuf>, Open),
        env: Vec<(String, String)>,
    ) -> Receiver<Msg> {
        let (tx, rx) = channel(4);
        let fail = |err: String| {
            _ = tx.try_send(Msg::AgentStarted {
                id,
                result: Err(err),
            });
        };
        if !cwd.is_dir() {
            fail(format!("{}: not a directory", cwd.display()));
            return rx;
        }
        let mut child = match command_in(&command, args, &cwd, env, Stdio::piped()) {
            Ok(child) => child,
            Err(err) => {
                fail(format!("{command}: {err}"));
                return rx;
            }
        };
        let pid = child.id().expect("a spawned child has a pid") as libc::pid_t;
        let (stdin, stdout, stderr) =
            (child.stdin.take(), child.stdout.take(), child.stderr.take());
        let (stdin, stdout, stderr) = (stdin.unwrap(), stdout.unwrap(), stderr.unwrap());
        let (exited, deletable) = (Arc::new(AtomicBool::new(false)), Arc::default());
        let pending = Pending::default();
        let open = Connection::default();
        self.handles.insert(
            id,
            Handle {
                pid,
                exited: exited.clone(),
                deletable: Arc::clone(&deletable),
                pending: pending.clone(),
                connection: open.clone(),
            },
        );
        let streams = (stdin, stdout);
        let process = (pid, exited.clone(), deletable);
        let connection = tokio::spawn(connect(
            id,
            (cwd, roots, session),
            streams,
            process,
            pending,
            open,
            tx.clone(),
        ));
        tokio::spawn(async move {
            let mut reader = BufReader::new(stderr);
            let mut bytes = Vec::new();
            while let Ok(count) = reader.read_until(b'\n', &mut bytes).await {
                if count == 0 {
                    break;
                }
                if bytes.last() == Some(&b'\n') {
                    bytes.pop();
                    if bytes.last() == Some(&b'\r') {
                        bytes.pop();
                    }
                }
                let line = String::from_utf8_lossy(&bytes).into_owned();
                bytes.clear();
                if tx.send(Msg::AgentStderr { id, line }).await.is_err() {
                    break;
                }
            }
            let status = child.wait().await;
            // A failed start is reported before the exit. shortcut: a process the agent left
            // holding its stdout delays the exit until it closes or the agent is killed; time
            // this out if wrappers ever leave such processes.
            _ = connection.await;
            exited.store(true, Ordering::SeqCst);
            match status {
                Ok(status) => _ = tx.send(exit(id, status)).await,
                Err(err) => tracing::warn!("waiting for the agent: {err}"),
            }
        });
        rx
    }

    /// Starts the process here, so a quit right after can kill it; the rest runs in a task.
    fn list(
        &mut self,
        request: u64,
        command: String,
        args: Vec<String>,
        cwd: PathBuf,
        env: Vec<(String, String)>,
    ) -> Receiver<Msg> {
        let (tx, rx) = channel(1);
        let fail = |err: String| {
            _ = tx.try_send(Msg::AgentListed {
                request,
                result: Err(err),
            });
        };
        if !cwd.is_dir() {
            fail(format!("{}: not a directory", cwd.display()));
            return rx;
        }
        // No log buffer for a throwaway process, so its stderr goes nowhere.
        let mut child = match command_in(&command, args, &cwd, env, Stdio::null()) {
            Ok(child) => child,
            Err(err) => {
                fail(format!("{command}: {err}"));
                return rx;
            }
        };
        let pid = child.id().expect("a spawned child has a pid") as libc::pid_t;
        let (stdin, stdout) = (child.stdin.take().unwrap(), child.stdout.take().unwrap());
        let exited = Arc::new(AtomicBool::new(false));
        (self.lists.lock().unwrap()).insert(request, (pid, exited.clone()));
        let lists = self.lists.clone();
        tokio::spawn(async move {
            // shortcut: an agent that never answers keeps its process until quit; time this out
            // if one ever hangs.
            let result = list_sessions(cwd, stdin, stdout).await;
            _ = tx.send(Msg::AgentListed { request, result }).await;
            // A quit that drained it terminates it instead.
            if lists.lock().unwrap().remove(&request).is_some()
                && let Some(group) = terminate(pid, &exited)
            {
                kill_after_grace(group);
            }
            _ = child.wait().await;
            exited.store(true, Ordering::SeqCst);
        });
        rx
    }
}

/// Initializes the agent and asks for its sessions in `cwd`, following `nextCursor`.
async fn list_sessions(
    cwd: PathBuf,
    stdin: ChildStdin,
    stdout: ChildStdout,
) -> Result<Vec<SessionInfo>, String> {
    let transport = ByteStreams::new(stdin.compat_write(), stdout.compat());
    let listed = Client
        .builder()
        .connect_with(transport, async |agent: ConnectionTo<Agent>| {
            let initialize = InitializeRequest::new(ProtocolVersion::V1)
                .client_info(Implementation::new("aiditor", env!("CARGO_PKG_VERSION")));
            let initialized = agent.send_request(initialize).block_task().await?;
            let capabilities = initialized.agent_capabilities.session_capabilities;
            if capabilities.list.is_none() {
                return Ok(Err("the agent can't list sessions".to_owned()));
            }
            let (mut sessions, mut cursor) = (Vec::new(), None);
            for _ in 0..LIST_PAGES {
                let page = ListSessionsRequest::new().cwd(cwd.clone()).cursor(cursor);
                let page = agent.send_request(page).block_task().await?;
                sessions.extend(page.sessions);
                cursor = page.next_cursor;
                if cursor.is_none() {
                    break;
                }
            }
            Ok(Ok(sessions))
        })
        .await;
    listed.map_err(|err| err.to_string())?
}

/// Starts `command` in `cwd` leading its own process group, with piped stdin and stdout.
fn command_in(
    command: &str,
    args: Vec<String>,
    cwd: &Path,
    env: Vec<(String, String)>,
    stderr: Stdio,
) -> std::io::Result<Child> {
    // With `PATH` in `env`, the program is looked up on that `PATH`.
    Command::new(command)
        .args(args)
        .current_dir(cwd)
        .envs(env)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(stderr)
        .process_group(0)
        .kill_on_drop(true)
        .spawn()
}

/// Initializes the agent and opens a session in `cwd`, with `roots` if it takes them, then
/// forwards the agent's requests until it closes its stdout, and then terminates it, so a
/// session that stops speaking still exits.
async fn connect(
    id: AgentId,
    (cwd, mut roots, open_as): (PathBuf, Vec<PathBuf>, Open),
    (stdin, stdout): (ChildStdin, ChildStdout),
    (pid, exited, deletable): (libc::pid_t, Arc<AtomicBool>, Arc<AtomicBool>),
    pending: Pending,
    open: Connection,
    tx: Sender<Msg>,
) {
    let (requests, updates) = (tx.clone(), tx.clone());
    let transport = ByteStreams::new(stdin.compat_write(), stdout.compat());
    let mut started = false;
    let result = Client
        .builder()
        .on_receive_request(
            async move |body: AgentRequest, responder: Responder, _| {
                let request = responder.id().clone();
                let cancellation = responder.cancellation();
                let (settled, answered) = oneshot::channel();
                (pending.lock().unwrap()).insert(request.clone(), (responder, settled));
                // `$/cancel_request` withdraws it: answered here, its block closed by the core.
                let (pending, withdrawn, at) = (pending.clone(), requests.clone(), request.clone());
                tokio::spawn(async move {
                    tokio::select! {
                        () = cancellation.cancelled() => {}
                        _ = answered => return,
                    }
                    let responder = pending.lock().unwrap().remove(&at);
                    if let Some((responder, _)) = responder {
                        let cancelled = agent_client_protocol::Error::request_cancelled();
                        _ = responder.respond_with_error(cancelled);
                        _ = withdrawn
                            .send(Msg::AgentWithdrawn { id, request: at })
                            .await;
                    }
                });
                _ = (requests.send(Msg::AgentRequest {
                    id,
                    request,
                    body: Request(Box::new(body)),
                }))
                .await;
                Ok(())
            },
            agent_client_protocol::on_receive_request!(),
        )
        .on_receive_notification(
            async move |update: SessionNotification, _| {
                let update = Box::new(update);
                _ = updates.send(Msg::AgentUpdate { id, update }).await;
                Ok(())
            },
            agent_client_protocol::on_receive_notification!(),
        )
        .connect_with(transport, async |agent: ConnectionTo<Agent>| {
            // The default capabilities offer neither fs nor terminals: agents use the disk. The
            // `_meta` extension asks for command output in tool calls (#100).
            let meta = serde_json::json!({"terminal_output": true});
            let elicitation =
                ElicitationCapabilities::new().form(ElicitationFormCapabilities::new());
            let capabilities = (ClientCapabilities::new().meta(meta.as_object().cloned()))
                .elicitation(elicitation);
            let initialize = InitializeRequest::new(ProtocolVersion::V1)
                .client_capabilities(capabilities)
                .client_info(Implementation::new("aiditor", env!("CARGO_PKG_VERSION")));
            let initialized = agent.send_request(initialize).block_task().await?;
            let capabilities = &initialized.agent_capabilities;
            if capabilities
                .session_capabilities
                .additional_directories
                .is_none()
                && !roots.is_empty()
            {
                roots.clear();
                _ = tx.send(Msg::AgentUnscoped { id }).await;
            }
            let delete = capabilities.session_capabilities.delete.is_some();
            deletable.store(delete, Ordering::SeqCst);
            // A rollback is a fork at its message; the core deletes the source.
            let open_as = match open_as {
                Open::Rollback(source, at) => Open::Fork(source, Some(at)),
                open_as => open_as,
            };
            let refusal = match &open_as {
                Open::Fork(..) if capabilities.session_capabilities.fork.is_none() => {
                    Some("agent can't fork sessions")
                }
                Open::Load(_) | Open::Fork(..) if !capabilities.load_session => {
                    Some("agent can't load sessions")
                }
                _ => None,
            };
            // The core kills an agent whose start failed, as after an error answer.
            if let Some(refusal) = refusal {
                let result = Err(refusal.into());
                _ = tx.send(Msg::AgentStarted { id, result }).await;
                return Ok(());
            }
            let load = match open_as {
                Open::New => None,
                Open::Load(session) => Some(session),
                Open::Rollback(..) => unreachable!("made a fork above"),
                // The fork replays nothing, so loading it shows the history it kept.
                Open::Fork(source, at) => {
                    let meta = at.map(|at| {
                        let fork = serde_json::json!({"version": 1, "messageId": at.0});
                        serde_json::json!({"jetbrains": {"air": {"fork": fork}}})
                    });
                    let fork = ForkSessionRequest::new(source, cwd.clone())
                        .additional_directories(roots.clone())
                        .meta(meta.and_then(|m| m.as_object().cloned()));
                    Some(agent.send_request(fork).block_task().await?.session_id)
                }
            };
            let session = match load {
                None => {
                    let new = NewSessionRequest::new(cwd).additional_directories(roots);
                    agent.send_request(new).block_task().await?
                }
                // The replayed history arrives as notifications before this answer.
                Some(session) => {
                    let load =
                        LoadSessionRequest::new(session.clone(), cwd).additional_directories(roots);
                    let loaded = agent.send_request(load).block_task().await?;
                    (NewSessionResponse::new(session).modes(loaded.modes))
                        .config_options(loaded.config_options)
                }
            };
            started = true;
            // Before the core hears of it, so its first prompt finds the connection.
            *open.lock().unwrap() = Some((agent.clone(), tx.clone()));
            _ = (tx.send(Msg::AgentStarted {
                id,
                result: Ok(session),
            }))
            .await;
            agent.incoming_closed().await;
            Ok(())
        })
        .await;
    open.lock().unwrap().take();
    if started && let Some(group) = terminate(pid, &exited) {
        kill_after_grace(group);
    }
    if let Err(err) = result {
        let err = err.to_string();
        if started {
            tracing::warn!("agent connection: {err}");
        } else {
            _ = (tx.send(Msg::AgentStarted {
                id,
                result: Err(err),
            }))
            .await;
        }
    }
}

fn exit(id: AgentId, status: ExitStatus) -> Msg {
    Msg::AgentExited {
        id,
        code: status.code().map_or(1, |code| code as u32),
        signal: status.signal().map(|signal| {
            // `strsignal` returns a static or thread-local string, valid until the next call.
            unsafe { CStr::from_ptr(libc::strsignal(signal)) }
                .to_string_lossy()
                .into_owned()
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use agent_client_protocol::schema::v1::ImageContent;

    #[test]
    fn base64_pads_each_tail() {
        let encoded: Vec<_> = ["", "f", "fo", "foo", "foob"]
            .map(|s| base64(s.as_bytes()))
            .into();
        assert_eq!(encoded, ["", "Zg==", "Zm8=", "Zm9v", "Zm9vYg=="]);
        assert_eq!(base64(&[0xfb, 0xff]), "+/8=");
    }

    #[tokio::test]
    async fn images_are_read_from_their_uri_or_fail_the_prompt() {
        let dir = tempfile::tempdir().unwrap();
        let cat = dir.path().join("cat.png");
        std::fs::write(&cat, b"foo").unwrap();
        let image = |path: &Path| {
            let uri = format!("file://{}", path.display());
            ContentBlock::Image(ImageContent::new("", "image/png").uri(uri))
        };
        let request = PromptRequest::new("s", vec!["look".into(), image(&cat)]);
        let read = read_images(request).await.unwrap();
        let ContentBlock::Image(image_read) = &read.prompt[1] else {
            panic!("an image");
        };
        assert_eq!(image_read.data, "Zm9v");
        let gone = PromptRequest::new("s", vec![image(&dir.path().join("gone.png"))]);
        assert!(
            read_images(gone)
                .await
                .unwrap_err()
                .starts_with("can't read")
        );
    }
}
