//! PTY processes, each with its own bounded output channel.

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::io::{self, Read};
use std::os::fd::RawFd;
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use aiditor_core::{Msg, TerminalOp};
use portable_pty::{CommandBuilder, ExitStatus, PtySize, native_pty_system};
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

/// Return this terminal's receiver: started, then output, then exit.
pub fn spawn(op: TerminalOp) -> Receiver<Msg> {
    let (tx, rx) = channel(4);
    thread::spawn(move || {
        let TerminalOp::Spawn {
            id,
            command,
            cwd,
            env,
            rows,
            cols,
        } = op;
        let start = || {
            if !cwd.is_dir() {
                return Err(format!("{}: not a directory", cwd.display()));
            }
            let mut command = match command {
                Some(args) if args.is_empty() => return Err("empty terminal command".into()),
                Some(args) => CommandBuilder::from_argv(args.into_iter().map(Into::into).collect()),
                None => CommandBuilder::new_default_prog(),
            };
            command.cwd(cwd);
            command.env_remove("SHLVL");
            for (key, value) in terminal_env(std::env::vars_os(), env) {
                command.env(key, value);
            }
            let pair = native_pty_system()
                .openpty(PtySize {
                    rows,
                    cols,
                    pixel_width: 0,
                    pixel_height: 0,
                })
                .map_err(|err| err.to_string())?;
            if let Some(fd) = pair.master.as_raw_fd()
                && let Err(err) = set_iutf8(fd)
            {
                tracing::warn!("setting terminal IUTF8: {err}");
            }
            let reader = pair
                .master
                .try_clone_reader()
                .map_err(|err| err.to_string())?;
            let child = pair
                .slave
                .spawn_command(command)
                .map_err(|err| err.to_string())?;
            drop(pair.slave);
            Ok((child, reader, pair.master))
        };
        let (mut child, reader, master) = match start() {
            Ok(process) => process,
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
        let (exited_tx, exited_rx) = mpsc::channel();
        thread::spawn(move || {
            let fd = master.as_raw_fd().expect("Unix PTY has a file descriptor");
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

fn read_output(
    mut reader: impl Read,
    fd: RawFd,
    id: aiditor_core::TerminalId,
    tx: Sender<Msg>,
    exited_rx: mpsc::Receiver<ExitStatus>,
) {
    let mut exited_rx = Some(exited_rx);
    let mut exit = None;
    let mut grace = Duration::from_millis(100);
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
                    exit = Some(status);
                    exited_rx = None;
                }
                Err(mpsc::TryRecvError::Disconnected) => exited_rx = None,
                Err(mpsc::TryRecvError::Empty) => {}
            }
        }
        if (eof || grace.is_zero())
            && let Some(status) = exit.take()
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
        // Only waiting for PTY input consumes the grace; output backpressure never does.
        let start = Instant::now();
        let timeout = if exit.is_some() {
            grace.as_millis().max(1) as i32
        } else {
            100
        };
        let ready = unsafe { libc::poll(&mut poll, 1, timeout) };
        if exit.is_some() {
            grace = grace.saturating_sub(start.elapsed());
        }
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
                if tx
                    .blocking_send(Msg::TerminalOutput {
                        id,
                        bytes: bytes[..len].to_vec(),
                    })
                    .is_err()
                {
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
