//! PTY processes, each with its own bounded output channel.

use std::collections::{BTreeMap, HashMap};
use std::ffi::OsString;
use std::fs::File;
use std::io::{self, Read, Write};
use std::os::fd::{AsRawFd, BorrowedFd, RawFd};
use std::path::PathBuf;
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use aiditor_core::{Msg, TerminalId, TerminalOp};
use portable_pty::{CommandBuilder, ExitStatus, MasterPty, PtySize, native_pty_system};
use tokio::sync::mpsc::{Receiver, Sender, channel};

fn terminal_env(
    base: impl Iterator<Item = (OsString, OsString)>,
    extra: Vec<(String, String)>,
) -> Vec<(OsString, OsString)> {
    let mut env: BTreeMap<_, _> = base.collect();
    env.extend(extra.into_iter().map(|(k, v)| (k.into(), v.into())));
    for (key, value) in [
        ("TERM", "xterm-256color"),
        ("COLORTERM", "truecolor"),
        ("TERM_PROGRAM", "aiditor"),
        ("TERM_PROGRAM_VERSION", env!("CARGO_PKG_VERSION")),
    ] {
        env.insert(key.into(), value.into());
    }
    env.entry("LANG".into()).or_insert("en_US.UTF-8".into());
    env.remove(&OsString::from("SHLVL"));
    env.into_iter().collect()
}

/// The spawned terminals, each one's PTY master and input queue by id.
#[derive(Default)]
pub struct Terminals(HashMap<TerminalId, Handle>);

struct Handle {
    master: Box<dyn MasterPty + Send>,
    input: mpsc::Sender<Vec<u8>>,
}

impl Terminals {
    /// Run `op` without blocking; a spawn returns its terminal's receiver: started, then output,
    /// then exit.
    pub fn run(&mut self, op: TerminalOp) -> Option<Receiver<Msg>> {
        match op {
            TerminalOp::Spawn {
                id,
                command,
                cwd,
                env,
                rows,
                cols,
            } => Some(self.spawn(id, command, cwd, env, size(rows, cols))),
            TerminalOp::Write { id, bytes } => {
                if let Some(terminal) = self.0.get(&id) {
                    _ = terminal.input.send(bytes);
                }
                None
            }
            TerminalOp::Resize { id, rows, cols } => {
                if let Some(terminal) = self.0.get(&id)
                    && let Err(err) = terminal.master.resize(size(rows, cols))
                {
                    tracing::warn!("resizing terminal: {err}");
                }
                None
            }
        }
    }

    /// Opens the PTY here, so input and resizes have somewhere to go at once; the rest runs on
    /// the terminal's own thread.
    fn spawn(
        &mut self,
        id: TerminalId,
        command: Option<Vec<String>>,
        cwd: PathBuf,
        env: Vec<(String, String)>,
        size: PtySize,
    ) -> Receiver<Msg> {
        let (tx, rx) = channel(4);
        let open = || {
            let pair = native_pty_system()
                .openpty(size)
                .map_err(|err| err.to_string())?;
            let fd = pair
                .master
                .as_raw_fd()
                .expect("Unix PTY has a file descriptor");
            if let Err(err) = set_iutf8(fd) {
                tracing::warn!("setting terminal IUTF8: {err}");
            }
            let reader = dup(fd).map_err(|err| err.to_string())?;
            let writer = dup(fd).map_err(|err| err.to_string())?;
            Ok::<_, String>((pair, reader, writer))
        };
        let (pair, reader, mut writer) = match open() {
            Ok(pty) => pty,
            Err(err) => {
                _ = tx.try_send(Msg::TerminalSpawned {
                    id,
                    result: Err(err),
                });
                return rx;
            }
        };
        let (input, input_rx) = mpsc::channel::<Vec<u8>>();
        self.0.insert(
            id,
            Handle {
                master: pair.master,
                input,
            },
        );
        let slave = pair.slave;
        thread::spawn(move || {
            let start = || {
                if !cwd.is_dir() {
                    return Err(format!("{}: not a directory", cwd.display()));
                }
                let mut command = match command {
                    Some(args) if args.is_empty() => return Err("empty terminal command".into()),
                    Some(args) => {
                        CommandBuilder::from_argv(args.into_iter().map(Into::into).collect())
                    }
                    None => CommandBuilder::new_default_prog(),
                };
                command.cwd(cwd);
                command.env_remove("SHLVL");
                for (key, value) in terminal_env(std::env::vars_os(), env) {
                    command.env(key, value);
                }
                slave.spawn_command(command).map_err(|err| err.to_string())
            };
            let result = start();
            drop(slave);
            let mut child = match result {
                Ok(child) => child,
                Err(err) => {
                    _ = tx.blocking_send(Msg::TerminalSpawned {
                        id,
                        result: Err(err),
                    });
                    return;
                }
            };
            if tx
                .blocking_send(Msg::TerminalSpawned { id, result: Ok(()) })
                .is_err()
            {
                return;
            }
            // Blocks only this thread when the program isn't reading; a write error means it's gone.
            thread::spawn(move || {
                for bytes in input_rx {
                    if writer.write_all(&bytes).is_err() {
                        break;
                    }
                }
            });
            let (exited_tx, exited_rx) = mpsc::channel();
            thread::spawn(move || {
                let fd = reader.as_raw_fd();
                read_output(reader, fd, id, tx, exited_rx);
            });
            match child.wait() {
                Ok(status) => {
                    _ = exited_tx.send(status);
                }
                Err(err) => tracing::warn!("waiting for terminal: {err}"),
            }
        });
        rx
    }
}

fn size(rows: u16, cols: u16) -> PtySize {
    PtySize {
        rows,
        cols,
        pixel_width: 0,
        pixel_height: 0,
    }
}

/// A duplicate of the master's fd, so each thread owns the fd it uses.
fn dup(fd: RawFd) -> io::Result<File> {
    // The fd belongs to the open master, which outlives this call.
    unsafe { BorrowedFd::borrow_raw(fd) }
        .try_clone_to_owned()
        .map(File::from)
}

fn read_output(
    mut reader: impl Read,
    fd: RawFd,
    id: TerminalId,
    tx: Sender<Msg>,
    exited_rx: mpsc::Receiver<ExitStatus>,
) {
    let mut exited_rx = Some(exited_rx);
    let mut exit = None;
    let mut eof = false;
    let mut bytes = vec![0; 64 * 1024];
    loop {
        if let Some(rx) = &exited_rx {
            let status = if eof {
                rx.recv().map_err(|_| mpsc::TryRecvError::Disconnected)
            } else {
                rx.try_recv()
            };
            match status {
                Ok(status) => {
                    exit = Some((status, Instant::now() + Duration::from_millis(100)));
                    exited_rx = None;
                }
                Err(mpsc::TryRecvError::Disconnected) => exited_rx = None,
                Err(mpsc::TryRecvError::Empty) => {}
            }
        }
        if (eof
            || exit
                .as_ref()
                .is_some_and(|(_, deadline)| Instant::now() >= *deadline))
            && let Some((status, _)) = exit.take()
            && tx
                .blocking_send(Msg::TerminalExited {
                    id,
                    code: status.exit_code(),
                    signal: status.signal().map(str::to_owned),
                })
                .is_err()
        {
            return;
        }
        if eof {
            return;
        }
        let mut poll = libc::pollfd {
            fd,
            events: libc::POLLIN,
            revents: 0,
        };
        let timeout = exit.as_ref().map_or(100, |(_, deadline)| {
            deadline
                .saturating_duration_since(Instant::now())
                .as_millis()
                .max(1) as i32
        });
        let ready = unsafe { libc::poll(&mut poll, 1, timeout) };
        match ready {
            0 => continue,
            -1 => {
                let err = io::Error::last_os_error();
                if err.kind() == io::ErrorKind::Interrupted {
                    continue;
                }
                tracing::warn!("polling terminal output: {err}");
                eof = true;
                continue;
            }
            _ => {}
        }
        match reader.read(&mut bytes) {
            Ok(0) => {
                eof = true;
            }
            Ok(len) => {
                let msg = Msg::TerminalOutput {
                    id,
                    bytes: bytes[..len].to_vec(),
                };
                let send_start = Instant::now();
                let result = tx.blocking_send(msg);
                // Pause only output delivery; reading and copying still consume the grace.
                if let Some((_, deadline)) = &mut exit {
                    *deadline += send_start.elapsed();
                }
                if result.is_err() {
                    return;
                }
            }
            Err(err) if err.kind() == io::ErrorKind::Interrupted => continue,
            Err(err) if err.raw_os_error() == Some(libc::EIO) => {
                eof = true;
            }
            Err(err) => {
                tracing::warn!("reading terminal output: {err}");
                eof = true;
            }
        }
    }
}

fn set_iutf8(fd: std::os::fd::RawFd) -> io::Result<()> {
    let mut attrs = std::mem::MaybeUninit::uninit();
    // The fd belongs to the open PTY, and tcgetattr initializes attrs on success.
    unsafe {
        if libc::tcgetattr(fd, attrs.as_mut_ptr()) != 0 {
            return Err(io::Error::last_os_error());
        }
        let mut attrs = attrs.assume_init();
        attrs.c_iflag |= libc::IUTF8;
        if libc::tcsetattr(fd, libc::TCSANOW, &attrs) != 0 {
            return Err(io::Error::last_os_error());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn exit_grace_includes_time_spent_reading_ready_output() {
        use std::io::Write;
        use std::os::fd::AsRawFd;
        use std::os::unix::net::UnixStream;

        struct SlowRead(UnixStream);
        impl Read for SlowRead {
            fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
                thread::sleep(Duration::from_millis(5));
                self.0.read(bytes)
            }
        }

        let (reader, mut writer) = UnixStream::pair().unwrap();
        let fd = reader.as_raw_fd();
        let producer = thread::spawn(move || while writer.write_all(&[b'x'; 1024]).is_ok() {});
        let id = aiditor_core::TerminalId(7);
        let (tx, mut rx) = channel(4);
        let (exited_tx, exited_rx) = mpsc::channel();
        exited_tx.send(ExitStatus::with_exit_code(0)).unwrap();
        let reader = thread::spawn(move || read_output(SlowRead(reader), fd, id, tx, exited_rx));
        let result = tokio::time::timeout(Duration::from_secs(1), async {
            let mut total = 0;
            loop {
                match rx.recv().await.unwrap() {
                    Msg::TerminalOutput { bytes, .. } => total += bytes.len(),
                    msg => {
                        assert_eq!(
                            msg,
                            Msg::TerminalExited {
                                id,
                                code: 0,
                                signal: None
                            }
                        );
                        return total;
                    }
                }
            }
        })
        .await;
        drop(rx);
        reader.join().unwrap();
        producer.join().unwrap();
        assert!(result.expect("ready output extended the exit grace") > 0);
    }

    #[test]
    fn environment_applies_overrides_and_terminal_defaults() {
        let build = |base: &[(&str, &str)], extra: &[(&str, &str)]| {
            terminal_env(
                base.iter().map(|(k, v)| ((*k).into(), (*v).into())),
                extra
                    .iter()
                    .map(|(k, v)| (k.to_string(), v.to_string()))
                    .collect(),
            )
            .into_iter()
            .collect::<BTreeMap<_, _>>()
        };
        let env = build(
            &[("SHLVL", "4"), ("FOO", "base")],
            &[("FOO", "bar"), ("TERM", "bad"), ("SHLVL", "9")],
        );
        for (key, value) in [
            ("TERM", "xterm-256color"),
            ("COLORTERM", "truecolor"),
            ("TERM_PROGRAM", "aiditor"),
            ("TERM_PROGRAM_VERSION", env!("CARGO_PKG_VERSION")),
            ("LANG", "en_US.UTF-8"),
            ("FOO", "bar"),
        ] {
            assert_eq!(env.get(&OsString::from(key)), Some(&OsString::from(value)));
        }
        assert_eq!(env.get(&OsString::from("SHLVL")), None);
        assert_eq!(
            build(&[("LANG", "et_EE.UTF-8")], &[])[&OsString::from("LANG")],
            "et_EE.UTF-8"
        );
        assert_eq!(build(&[], &[("LANG", "C")])[&OsString::from("LANG")], "C");
    }
}
