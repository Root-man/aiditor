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
