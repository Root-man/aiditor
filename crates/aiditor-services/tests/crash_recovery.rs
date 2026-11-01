//! Edit a file, kill the editor, relaunch it and recover the edit, through the real services.

use std::path::Path;
use std::process::{Child, Command};
use std::time::{Duration, Instant};
use std::{fs, thread};

/// The editor process, killed when dropped so a failed test leaves none behind.
struct Editor(Child);

impl Drop for Editor {
    fn drop(&mut self) {
        _ = self.0.kill();
        _ = self.0.wait();
    }
}

fn launch(dir: &Path, file: &Path, script: &str) -> Editor {
    let child = Command::new(env!("CARGO_BIN_EXE_aiditor-headless"))
        .args([dir, file])
        .arg(script)
        .spawn()
        .unwrap();
    Editor(child)
}

/// Waits up to 30 s for `done`, polling.
fn wait_for(what: &str, mut done: impl FnMut() -> bool) {
    let start = Instant::now();
    while !done() {
        assert!(
            start.elapsed() < Duration::from_secs(30),
            "timed out: {what}"
        );
        thread::sleep(Duration::from_millis(50));
    }
}

#[test]
fn a_killed_session_is_recovered_on_relaunch() {
    let root = tempfile::tempdir().unwrap();
    let (dir, file) = (root.path().join("recovery"), root.path().join("a.txt"));
    fs::write(&file, "world\n").unwrap();

    let mut first = launch(&dir, &file, "hello SPC");
    let written =
        || fs::read_dir(&dir).is_ok_and(|e| e.flatten().any(|e| e.path().extension().is_none()));
    wait_for("the auto-save", written);
    first.0.kill().unwrap();
    first.0.wait().unwrap();
    assert_eq!(fs::read_to_string(&file).unwrap(), "world\n");

    // Accept the recovery prompt, save, quit.
    let mut second = launch(&dir, &file, "RET C-x C-s C-x C-c");
    let mut status = None;
    wait_for("the relaunch to quit", || {
        status = second.0.try_wait().unwrap();
        status.is_some()
    });
    assert!(status.unwrap().success());
    assert_eq!(fs::read_to_string(&file).unwrap(), "hello world\n");
    assert_eq!(
        fs::read_dir(&dir).unwrap().count(),
        0,
        "recovery files left"
    );
}

#[test]
fn an_obstructed_auto_save_retries_without_input_and_recovers_on_relaunch() {
    use aiditor_core::{Core, Effect, Msg, RecoveryOp};
    let root = tempfile::tempdir().unwrap();
    let (dir, file) = (root.path().join("recovery"), root.path().join("a.txt"));
    fs::write(&file, "world\n").unwrap();
    let mut probe = Core::default();
    probe.open(&file, aiditor_services::files::read(&file).unwrap());
    probe.handle(
        Instant::now(),
        Msg::Insert {
            text: "hello ".into(),
            replace: None,
        },
    );
    let number = probe
        .handle(Instant::now(), Msg::AutoSave)
        .into_iter()
        .find_map(|e| match e {
            Effect::Recovery(RecoveryOp::AutoSave { buffer, .. }) => Some(buffer.number()),
            _ => None,
        })
        .unwrap();
    let mut first = launch(&dir, &file, "hello SPC");
    let tmp = dir.join(format!("{}-{number}.tmp", first.0.id()));
    fs::create_dir_all(&tmp).unwrap();
    thread::sleep(Duration::from_secs(3));
    assert_eq!(fs::read_dir(&dir).unwrap().count(), 1);
    fs::remove_dir(&tmp).unwrap();
    let recovery = dir.join(format!("{}-{number}", first.0.id()));
    wait_for("retry without input", || {
        fs::read_to_string(&recovery).is_ok_and(|s| s.ends_with("\nhello world\n"))
    });
    assert_eq!(fs::read_to_string(&file).unwrap(), "world\n");
    first.0.kill().unwrap();
    first.0.wait().unwrap();
    let mut second = launch(&dir, &file, "RET C-x C-s C-x C-c");
    let mut status = None;
    wait_for("recovered retry to save and quit", || {
        status = second.0.try_wait().unwrap();
        status.is_some()
    });
    assert!(status.unwrap().success());
    assert_eq!(fs::read_to_string(&file).unwrap(), "hello world\n");
    assert_eq!(fs::read_dir(&dir).unwrap().count(), 0);
}
