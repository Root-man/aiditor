//! The core and services without GPUI, driven by a key script, for end-to-end tests on CI.
//!
//! usage: aiditor-headless RECOVERY_DIR FILE SCRIPT
//!
//! Opens FILE, offers the recovery files in RECOVERY_DIR as the app does at launch, feeds the
//! `kbd`-style SCRIPT, then fires the core's timers on the real clock until the core quits.

use std::path::PathBuf;
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::thread::JoinHandle;
use std::time::Instant;

use aiditor_core::key::{Key, KeyCode, parse_keys};
use aiditor_core::{Core, Effect, GrepOp, Msg, RecoveryOp};
use aiditor_services::{files, grep, recovery};

/// The recovery service, until the core quits.
type Service = Option<(Sender<RecoveryOp>, JoinHandle<()>)>;

fn main() {
    let [dir, file, script]: [String; 3] = (std::env::args().skip(1).collect::<Vec<_>>())
        .try_into()
        .expect("usage: aiditor-headless RECOVERY_DIR FILE SCRIPT");
    let (dir, file) = (PathBuf::from(dir), std::path::absolute(file).unwrap());
    let mut core = Core::default();
    core.open(&file, files::read(&file).unwrap());
    let (tx, completions) = mpsc::channel();
    let mut service = Some(recovery::spawn(dir.clone(), move |msg| {
        _ = tx.send(msg);
    }));
    send(&mut core, &mut service, Msg::Recover(recovery::list(&dir)));
    for key in parse_keys(&script).unwrap() {
        // As the app's text input does, plain chars arrive as typed text.
        let msg = match key.code {
            KeyCode::Char(c) if key == Key::plain(key.code) => Msg::Insert {
                text: c.into(),
                replace: None,
            },
            _ => Msg::Key(key),
        };
        send(&mut core, &mut service, msg);
    }
    loop {
        let msg = match core.next_deadline() {
            Some(at) => {
                match completions.recv_timeout(at.saturating_duration_since(Instant::now())) {
                    Ok(msg) => msg,
                    Err(RecvTimeoutError::Timeout) => Msg::Tick,
                    Err(RecvTimeoutError::Disconnected) => break,
                }
            }
            None => match completions.recv() {
                Ok(msg) => msg,
                Err(_) => break,
            },
        };
        send(&mut core, &mut service, msg);
    }
}

/// Sends `msg` to the core and carries out its effects at once, so a save finishes before the
/// next key.
fn send(core: &mut Core, service: &mut Service, msg: Msg) {
    for effect in core.handle(Instant::now(), msg) {
        let reply = match effect {
            Effect::Terminal(_) | Effect::Agent(_) => continue,
            Effect::File(op) => files::run(op),
            // Runs to the end before the next key, so nothing is left to cancel.
            Effect::Grep(GrepOp::Search(op)) => {
                let mut replies = Vec::new();
                grep::search(&op, || false, |msg| replies.push(msg));
                for reply in replies {
                    send(core, service, reply);
                }
                continue;
            }
            Effect::Grep(GrepOp::Cancel) => continue,
            Effect::Recovery(op) => {
                let failure = match service.as_ref() {
                    Some((tx, _)) => tx
                        .send(op)
                        .err()
                        .and_then(|e| e.0.failed("Recovery service closed".into())),
                    None => op.failed("Recovery service unavailable".into()),
                };
                if let Some(msg) = failure {
                    send(core, service, msg);
                }
                continue;
            }
            Effect::Quit => {
                // Waits for queued recovery writes and deletes, as the app does before it exits.
                let (tx, thread) = service.take().unwrap();
                drop(tx);
                thread.join().unwrap();
                std::process::exit(0);
            }
            // Key scripts run against files nothing else touches.
            Effect::SetClipboard(_)
            | Effect::LoadConfiguration { .. }
            | Effect::Watch(_)
            | Effect::CaptureEnvironment { .. } => continue,
        };
        send(core, service, reply);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn omitted_configuration_keeps_file_and_search_effects_working() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("a.txt");
        std::fs::write(&file, "needle\n").unwrap();
        let mut core = Core::default();
        core.launch(dir.path(), dir.path());
        core.open(&file, files::read(&file).unwrap());
        let mut service = Some(recovery::spawn(dir.path().join("recovery"), |_| {}));
        for key in parse_keys("C-x C-r x C-x C-s C-x p g needle").unwrap() {
            send(&mut core, &mut service, Msg::Key(key));
        }
        assert_eq!(std::fs::read_to_string(&file).unwrap(), "xneedle\n");
        std::thread::sleep(std::time::Duration::from_millis(210));
        send(&mut core, &mut service, Msg::Tick);
        assert!(
            core.frame()
                .minibuffer
                .unwrap()
                .candidates
                .iter()
                .any(|s| s == "  1: xneedle")
        );
        for key in parse_keys("RET").unwrap() {
            send(&mut core, &mut service, Msg::Key(key));
        }
        assert_eq!(core.current().file.as_ref(), Some(&file));
        assert_eq!(core.point(), 1);
        let (tx, thread) = service.take().unwrap();
        drop(tx);
        thread.join().unwrap();
    }
}
