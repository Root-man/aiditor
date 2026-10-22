//! The core and services without GPUI, driven by a key script, for end-to-end tests on CI.
//!
//! usage: aiditor-headless RECOVERY_DIR FILE SCRIPT
//!
//! Opens FILE, offers the recovery files in RECOVERY_DIR as the app does at launch, feeds the
//! `kbd`-style SCRIPT, then fires the core's timers on the real clock until the core quits.

use std::path::PathBuf;
use std::sync::mpsc::Sender;
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
    let mut service = Some(recovery::spawn(dir.clone()));
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
    // Idle until a timer is due, or until killed when none is.
    loop {
        match core.next_deadline() {
            Some(at) => std::thread::sleep(at.saturating_duration_since(Instant::now())),
            None => std::thread::park(),
        }
        send(&mut core, &mut service, Msg::Tick);
    }
}

/// Sends `msg` to the core and carries out its effects at once, so a save finishes before the
/// next key.
fn send(core: &mut Core, service: &mut Service, msg: Msg) {
    for effect in core.handle(Instant::now(), msg) {
        let reply = match effect {
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
                _ = service.as_ref().unwrap().0.send(op);
                continue;
            }
            Effect::Quit => {
                // Waits for queued recovery writes and deletes, as the app does before it exits.
                let (tx, thread) = service.take().unwrap();
                drop(tx);
                thread.join().unwrap();
                std::process::exit(0);
            }
            Effect::SetClipboard(_) | Effect::LoadSettings => continue,
        };
        send(core, service, reply);
    }
}
