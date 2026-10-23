use std::path::Path;
use std::time::{Duration, Instant};

use aiditor_core::{FileOp, Msg, TerminalId, TerminalOp};
use aiditor_services::terminal::Terminals;
use aiditor_services::{channel, files};
use tokio::sync::mpsc::Receiver;
use tokio::time::timeout;

const ID: TerminalId = TerminalId(7);

fn spawn(
    terminals: &mut Terminals,
    cwd: &Path,
    command: Option<Vec<String>>,
    env: Vec<(String, String)>,
) -> Receiver<Msg> {
    terminals
        .run(TerminalOp::Spawn {
            id: ID,
            command,
            cwd: cwd.into(),
            env,
            rows: 24,
            cols: 80,
        })
        .expect("spawn returns a receiver")
}

fn start(cwd: &Path, command: Option<Vec<String>>, env: Vec<(String, String)>) -> Receiver<Msg> {
    spawn(&mut Terminals::default(), cwd, command, env)
}

fn sh(script: &str) -> Option<Vec<String>> {
    Some(vec!["/bin/sh".into(), "-c".into(), script.into()])
}

fn script(cwd: &Path, script: &str) -> Receiver<Msg> {
    start(cwd, sh(script), vec![])
}

fn write(terminals: &mut Terminals, bytes: &[u8]) {
    assert!(
        terminals
            .run(TerminalOp::Write {
                id: ID,
                bytes: bytes.into()
            })
            .is_none()
    );
}

/// The output until it holds `needle`.
async fn output_until(rx: &mut Receiver<Msg>, needle: &str) -> String {
    let mut output = Vec::new();
    while !String::from_utf8_lossy(&output).contains(needle) {
        match next(rx).await {
            Some(Msg::TerminalOutput { bytes, .. }) => output.extend(bytes),
            msg => panic!("expected {needle:?} in output, got {msg:?} after {output:?}"),
        }
    }
    String::from_utf8_lossy(&output).into()
}

async fn next(rx: &mut Receiver<Msg>) -> Option<Msg> {
    timeout(Duration::from_secs(10), rx.recv())
        .await
        .expect("terminal timed out")
}

async fn started(rx: &mut Receiver<Msg>) {
    assert_eq!(
        next(rx).await,
        Some(Msg::TerminalSpawned {
            id: ID,
            result: Ok(())
        })
    );
}

async fn output_until_exit(rx: &mut Receiver<Msg>, code: u32) -> Vec<u8> {
    let mut output = Vec::new();
    loop {
        match next(rx).await.expect("terminal closed before exit") {
            Msg::TerminalOutput { id, bytes } => {
                assert_eq!(id, ID);
                assert!((1..=64 * 1024).contains(&bytes.len()));
                output.extend(bytes);
            }
            msg => {
                assert_eq!(
                    msg,
                    Msg::TerminalExited {
                        id: ID,
                        code,
                        signal: None
                    }
                );
                return output;
            }
        }
    }
}

#[tokio::test]
async fn streams_output_before_real_exit_status() {
    let dir = tempfile::tempdir().unwrap();
    let mut rx = script(dir.path(), "printf a; printf b; exit 3");
    started(&mut rx).await;
    assert_eq!(output_until_exit(&mut rx, 3).await, b"ab");
    assert_eq!(next(&mut rx).await, None);
}

#[tokio::test]
async fn spawn_failures_close_the_stream() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("file");
    std::fs::write(&file, "not a directory").unwrap();
    for mut rx in [
        start(
            dir.path(),
            Some(vec!["/nonexistent/program".into()]),
            vec![],
        ),
        script(&dir.path().join("missing"), "echo wrong"),
        script(&file, "echo wrong"),
        start(dir.path(), Some(vec![]), vec![]),
    ] {
        let Some(Msg::TerminalSpawned { id, result }) = next(&mut rx).await else {
            panic!("expected spawn result");
        };
        assert_eq!(id, ID);
        assert!(!result.unwrap_err().is_empty());
        assert_eq!(next(&mut rx).await, None);
    }
}

#[tokio::test]
async fn large_output_arrives_whole_in_bounded_batches() {
    let dir = tempfile::tempdir().unwrap();
    let bytes: Vec<u8> = (0..1_000_000).map(|i| b'a' + (i % 26) as u8).collect();
    std::fs::write(dir.path().join("data"), &bytes).unwrap();
    let mut rx = script(dir.path(), "cat data");
    started(&mut rx).await;
    assert_eq!(output_until_exit(&mut rx, 0).await, bytes);
    assert_eq!(next(&mut rx).await, None);
}

#[tokio::test]
async fn exit_does_not_wait_for_a_background_job_holding_the_pty() {
    let dir = tempfile::tempdir().unwrap();
    let mut rx = script(dir.path(), "sleep 5 & exit 0");
    started(&mut rx).await;
    timeout(Duration::from_secs(1), output_until_exit(&mut rx, 0))
        .await
        .unwrap();
}

#[tokio::test]
async fn terminal_environment_and_utf8_flag_reach_the_child() {
    let dir = tempfile::tempdir().unwrap();
    let mut rx = start(
        dir.path(),
        Some(vec!["/bin/sh".into(), "-c".into(), "env; stty -a".into()]),
        vec![("FOO".into(), "bar".into())],
    );
    started(&mut rx).await;
    let output = String::from_utf8(output_until_exit(&mut rx, 0).await).unwrap();
    for line in [
        "TERM=xterm-256color".to_string(),
        "COLORTERM=truecolor".into(),
        "TERM_PROGRAM=aiditor".into(),
        format!("TERM_PROGRAM_VERSION={}", env!("CARGO_PKG_VERSION")),
        "FOO=bar".into(),
    ] {
        assert!(output.lines().any(|s| s == line), "{output}");
    }
    assert!(
        output
            .split_whitespace()
            .any(|s| s.trim_end_matches(';') == "iutf8"),
        "{output}"
    );
}

#[tokio::test]
async fn default_bash_reads_the_login_profile() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join(".bash_profile"), "echo \"$0\"; exit\n").unwrap();
    let mut rx = start(
        dir.path(),
        None,
        vec![
            ("SHELL".into(), "/bin/bash".into()),
            ("HOME".into(), dir.path().display().to_string()),
        ],
    );
    started(&mut rx).await;
    assert_eq!(
        String::from_utf8(output_until_exit(&mut rx, 0).await)
            .unwrap()
            .trim(),
        "-bash"
    );
}

#[tokio::test]
async fn default_zsh_reads_the_login_profile() {
    let Some(zsh) = ["/bin/zsh", "/usr/bin/zsh"]
        .into_iter()
        .find(|path| Path::new(path).is_file())
    else {
        eprintln!("zsh not installed; skipping login test");
        return;
    };
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join(".zprofile"), "echo \"$0\"; exit\n").unwrap();
    let mut rx = start(
        dir.path(),
        None,
        vec![
            ("SHELL".into(), zsh.into()),
            ("ZDOTDIR".into(), dir.path().display().to_string()),
        ],
    );
    started(&mut rx).await;
    assert_eq!(
        String::from_utf8(output_until_exit(&mut rx, 0).await)
            .unwrap()
            .trim(),
        "-zsh"
    );
}

#[tokio::test]
async fn backpressure_stalls_only_its_terminal_and_draining_resumes_it() {
    let dir = tempfile::tempdir().unwrap();
    let mut rx = script(dir.path(), "head -c 8000000 /dev/zero; touch done");
    tokio::time::sleep(Duration::from_millis(500)).await;
    assert!(!dir.path().join("done").exists());
    let (tx, mut shared) = channel();
    let cwd = dir.path().to_path_buf();
    tokio::task::spawn_blocking(move || {
        tx.blocking_send(files::run(FileOp::ListDir(cwd))).unwrap()
    });
    assert!(matches!(
        timeout(Duration::from_secs(1), shared.recv())
            .await
            .unwrap(),
        Some(Msg::Listed { result: Ok(_), .. })
    ));
    let mut other = script(dir.path(), "printf other");
    started(&mut other).await;
    assert_eq!(output_until_exit(&mut other, 0).await, b"other");
    started(&mut rx).await;
    assert_eq!(output_until_exit(&mut rx, 0).await, vec![0; 8_000_000]);
    assert!(dir.path().join("done").exists());
    assert_eq!(next(&mut rx).await, None);
}

#[tokio::test]
async fn buffered_output_precedes_exit_after_a_slow_receiver_resumes() {
    let dir = tempfile::tempdir().unwrap();
    let mut rx = script(
        dir.path(),
        "i=0; while [ $i -lt 10 ]; do printf abcdefghij; sleep 0.05; i=$((i+1)); done; touch done",
    );
    started(&mut rx).await;
    let done = dir.path().join("done");
    timeout(Duration::from_secs(10), async {
        while !done.exists() {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("terminal did not finish writing while its receiver was held");
    tokio::time::sleep(Duration::from_millis(150)).await;
    let bytes = output_until_exit(&mut rx, 0).await;
    assert_eq!(
        bytes.len(),
        100,
        "output before exit: {}",
        String::from_utf8_lossy(&bytes)
    );
    assert_eq!(bytes, b"abcdefghij".repeat(10));
    assert_eq!(next(&mut rx).await, None);
}

// macOS revokes the slave on the session leader's exit, so further writes fail.
#[cfg(target_os = "linux")]
#[tokio::test]
async fn exit_is_reported_while_a_background_job_keeps_writing() {
    let dir = tempfile::tempdir().unwrap();
    let mut rx = script(
        dir.path(),
        "(trap '' HUP; touch ready; i=0; while [ \"$i\" -lt 100 ]; do printf b; sleep 0.02; i=$((i + 1)); done) & while [ ! -f ready ]; do sleep 0.01; done; printf a; exit 0",
    );
    started(&mut rx).await;
    let mut bytes = timeout(Duration::from_secs(1), output_until_exit(&mut rx, 0))
        .await
        .unwrap();
    assert!(bytes.contains(&b'a'));
    let Some(Msg::TerminalOutput { id, bytes: more }) = next(&mut rx).await else {
        panic!("expected background output after exit");
    };
    assert_eq!(id, ID);
    bytes.extend(more);
    while let Some(msg) = next(&mut rx).await {
        let Msg::TerminalOutput { id, bytes: more } = msg else {
            panic!("expected background output, got {msg:?}");
        };
        assert_eq!(id, ID);
        bytes.extend(more);
    }
    assert_eq!(bytes.iter().filter(|&&b| b == b'a').count(), 1);
    assert_eq!(bytes.iter().filter(|&&b| b == b'b').count(), 100);
}

#[tokio::test]
async fn large_final_output_precedes_exit_with_a_slow_receiver() {
    let dir = tempfile::tempdir().unwrap();
    let mut rx = script(dir.path(), "head -c 1000000 /dev/zero");
    started(&mut rx).await;
    let mut total = 0;
    loop {
        match next(&mut rx).await.unwrap() {
            Msg::TerminalOutput { id, bytes } => {
                assert_eq!(id, ID);
                assert!((1..=64 * 1024).contains(&bytes.len()));
                assert!(bytes.iter().all(|&b| b == 0));
                total += bytes.len();
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
            msg => {
                assert_eq!(
                    msg,
                    Msg::TerminalExited {
                        id: ID,
                        code: 0,
                        signal: None
                    }
                );
                break;
            }
        }
    }
    assert_eq!(total, 1_000_000);
    assert_eq!(next(&mut rx).await, None);
}

#[cfg(target_os = "linux")]
#[tokio::test]
async fn exit_is_reported_during_a_background_output_flood() {
    let dir = tempfile::tempdir().unwrap();
    let mut rx = script(
        dir.path(),
        "(trap '' HUP; exec > /dev/tty; touch ready; exec yes b) & echo $! > writer.pid; while [ ! -f ready ]; do sleep 0.01; done; exit 0",
    );
    started(&mut rx).await;
    let result = timeout(Duration::from_secs(1), async {
        let mut total = 0;
        loop {
            match next(&mut rx).await.unwrap() {
                Msg::TerminalOutput { id, bytes } => {
                    assert_eq!(id, ID);
                    total += bytes.len();
                }
                msg => {
                    assert_eq!(
                        msg,
                        Msg::TerminalExited {
                            id: ID,
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
    let after_exit = if result.is_ok() {
        next(&mut rx).await
    } else {
        None
    };
    let pid = std::fs::read_to_string(dir.path().join("writer.pid")).unwrap();
    assert!(
        std::process::Command::new("kill")
            .args(["-TERM", pid.trim()])
            .status()
            .unwrap()
            .success()
    );
    assert!(result.expect("shell exit was delayed by continuous background output") > 0);
    assert!(matches!(after_exit, Some(Msg::TerminalOutput { id: ID, bytes }) if !bytes.is_empty()));
}

#[tokio::test]
async fn input_echoes_and_reaches_the_program() {
    let dir = tempfile::tempdir().unwrap();
    let mut terminals = Terminals::default();
    let mut rx = spawn(&mut terminals, dir.path(), Some(vec!["cat".into()]), vec![]);
    started(&mut rx).await;
    write(&mut terminals, b"hi\n");
    write(&mut terminals, b"\x04");
    let output = String::from_utf8(output_until_exit(&mut rx, 0).await).unwrap();
    assert_eq!(output.matches("hi").count(), 2, "{output:?}");
}

#[tokio::test]
async fn resize_reaches_the_program() {
    let dir = tempfile::tempdir().unwrap();
    let mut terminals = Terminals::default();
    let mut rx = spawn(&mut terminals, dir.path(), sh("read x; stty size"), vec![]);
    started(&mut rx).await;
    let resize = TerminalOp::Resize {
        id: ID,
        rows: 30,
        cols: 100,
    };
    assert!(terminals.run(resize).is_none());
    write(&mut terminals, b"\n");
    assert!(output_until_exit(&mut rx, 0).await.ends_with(b"30 100\r\n"));
}

#[tokio::test]
async fn ctrl_c_interrupts_the_foreground_job() {
    let dir = tempfile::tempdir().unwrap();
    let mut terminals = Terminals::default();
    let command = Some(vec!["sleep".into(), "30".into()]);
    let mut rx = spawn(&mut terminals, dir.path(), command, vec![]);
    started(&mut rx).await;
    write(&mut terminals, b"\x03");
    timeout(Duration::from_secs(5), async {
        loop {
            match next(&mut rx).await.expect("terminal closed before exit") {
                Msg::TerminalOutput { .. } => {}
                Msg::TerminalExited { code, signal, .. } => {
                    return assert!(code != 0 || signal.is_some());
                }
                msg => panic!("unexpected {msg:?}"),
            }
        }
    })
    .await
    .expect("sleep was not interrupted");
}

#[tokio::test]
async fn backspace_erases_a_whole_multibyte_char() {
    let dir = tempfile::tempdir().unwrap();
    let mut terminals = Terminals::default();
    let script = sh("read -r line; printf '<%s>' \"$line\"");
    let mut rx = spawn(&mut terminals, dir.path(), script, vec![]);
    started(&mut rx).await;
    write(&mut terminals, "a\u{e9}\x7f\n".as_bytes());
    output_until(&mut rx, "<a>").await;
}

#[tokio::test]
async fn a_large_write_to_a_program_that_does_not_read_returns_at_once() {
    let dir = tempfile::tempdir().unwrap();
    let mut terminals = Terminals::default();
    let script = sh("stty raw -echo; echo ready; sleep 5");
    let mut rx = spawn(&mut terminals, dir.path(), script, vec![]);
    started(&mut rx).await;
    output_until(&mut rx, "ready").await;
    let start = Instant::now();
    write(&mut terminals, &vec![b'x'; 1_000_000]);
    write(&mut terminals, b"more");
    assert!(start.elapsed() < Duration::from_secs(1));
}

#[test]
fn input_and_resize_for_an_unknown_terminal_are_ignored() {
    let mut terminals = Terminals::default();
    write(&mut terminals, b"x");
    let resize = TerminalOp::Resize {
        id: ID,
        rows: 30,
        cols: 100,
    };
    assert!(terminals.run(resize).is_none());
}

/// The numbers after the last `label ` in `output`.
fn pids_after(output: &str, label: &str) -> Vec<libc::pid_t> {
    let (_, rest) = output.rsplit_once(label).expect("labelled pids");
    rest.split_whitespace()
        .map_while(|word| word.parse().ok())
        .collect()
}

async fn gone(pid: libc::pid_t) {
    timeout(Duration::from_secs(5), async {
        // Signal 0 only checks the process exists.
        while unsafe { libc::kill(pid, 0) } == 0 {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap_or_else(|_| panic!("process {pid} still exists"));
}

/// Drains `rx` until the exit message, which must come from a signal.
async fn killed(rx: &mut Receiver<Msg>) {
    loop {
        match next(rx).await.expect("terminal closed before exit") {
            Msg::TerminalSpawned { .. } | Msg::TerminalOutput { .. } => {}
            Msg::TerminalExited { signal, .. } => return assert!(signal.is_some()),
            msg => panic!("unexpected {msg:?}"),
        }
    }
}

fn kill(terminals: &mut Terminals, id: TerminalId) {
    assert!(terminals.run(TerminalOp::Kill { id }).is_none());
}

#[tokio::test]
async fn kill_ends_the_shell_and_its_foreground_job() {
    let dir = tempfile::tempdir().unwrap();
    let mut terminals = Terminals::default();
    let bash = ["bash", "--norc", "--noprofile", "-i"];
    let command = Some(bash.map(Into::into).to_vec());
    let mut rx = spawn(&mut terminals, dir.path(), command, vec![]);
    started(&mut rx).await;
    // `fg` makes the job the foreground process group, apart from the shell's.
    write(&mut terminals, b"sleep 1000 & echo pids $$ $! ; fg\n");
    let output = output_until(&mut rx, "sleep 1000\r\n").await;
    let pids = pids_after(&output, "pids ");
    assert_eq!(pids.len(), 2, "{output:?}");
    kill(&mut terminals, ID);
    killed(&mut rx).await;
    for pid in pids {
        gone(pid).await;
    }
}

#[tokio::test]
async fn kill_ends_a_shell_that_ignores_hangup_and_term() {
    let dir = tempfile::tempdir().unwrap();
    let mut terminals = Terminals::default();
    let script = sh("trap '' HUP TERM; echo \"pid $$ \"; while :; do sleep 1; done");
    let mut rx = spawn(&mut terminals, dir.path(), script, vec![]);
    started(&mut rx).await;
    let pid = pids_after(&output_until(&mut rx, " \r\n").await, "pid ")[0];
    kill(&mut terminals, ID);
    killed(&mut rx).await;
    gone(pid).await;
    let resize = TerminalOp::Resize {
        id: ID,
        rows: 30,
        cols: 100,
    };
    assert!(terminals.run(resize).is_none());
}

#[tokio::test]
async fn kill_ends_a_foreground_job_that_outlives_its_shell() {
    let dir = tempfile::tempdir().unwrap();
    let mut terminals = Terminals::default();
    let bash = ["bash", "--norc", "--noprofile", "-i"];
    let mut rx = spawn(
        &mut terminals,
        dir.path(),
        Some(bash.map(Into::into).to_vec()),
        vec![],
    );
    started(&mut rx).await;
    // Ignored signals stay ignored across exec, so the shell dies of SIGHUP and the job doesn't.
    write(
        &mut terminals,
        b"sh -c \"trap '' HUP TERM; exec sleep 1000.3\"\n",
    );
    timeout(Duration::from_secs(5), async {
        while !std::process::Command::new("pgrep")
            .args(["-xf", "sleep 1000.3"])
            .status()
            .unwrap()
            .success()
        {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("the job started");
    kill(&mut terminals, ID);
    killed(&mut rx).await;
    none_running("sleep 1000.3").await;
}

/// Waits until no process has exactly `args` as its command line.
async fn none_running(args: &str) {
    timeout(Duration::from_secs(5), async {
        while std::process::Command::new("pgrep")
            .args(["-xf", args])
            .status()
            .unwrap()
            .success()
        {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap_or_else(|_| panic!("{args} still running"));
}

#[tokio::test]
async fn kill_right_after_spawn_ends_the_shell() {
    let dir = tempfile::tempdir().unwrap();
    let mut terminals = Terminals::default();
    let command = Some(vec!["sleep".into(), "1000.1".into()]);
    let mut rx = spawn(&mut terminals, dir.path(), command, vec![]);
    kill(&mut terminals, ID);
    killed(&mut rx).await;
    none_running("sleep 1000.1").await;
}

#[tokio::test]
async fn kill_all_right_after_spawn_ends_the_shell() {
    let dir = tempfile::tempdir().unwrap();
    let mut terminals = Terminals::default();
    let command = Some(vec!["sleep".into(), "1000.2".into()]);
    let _rx = spawn(&mut terminals, dir.path(), command, vec![]);
    terminals.kill_all();
    none_running("sleep 1000.2").await;
}

#[tokio::test]
async fn kill_all_leaves_no_shells() {
    let dir = tempfile::tempdir().unwrap();
    let mut terminals = Terminals::default();
    let mut pids = Vec::new();
    for (id, script) in [
        (TerminalId(1), "echo \"pid $$ \"; sleep 1000"),
        (
            TerminalId(2),
            "trap '' HUP TERM; echo \"pid $$ \"; sleep 1000",
        ),
    ] {
        let spawn = TerminalOp::Spawn {
            id,
            command: sh(script),
            cwd: dir.path().into(),
            env: vec![],
            rows: 24,
            cols: 80,
        };
        let mut rx = terminals.run(spawn).unwrap();
        let started = Msg::TerminalSpawned { id, result: Ok(()) };
        assert_eq!(next(&mut rx).await, Some(started));
        pids.push(pids_after(&output_until(&mut rx, " \r\n").await, "pid ")[0]);
        // Kept open, so the terminal's reader doesn't stop at a dropped receiver.
        std::mem::forget(rx);
    }
    let start = Instant::now();
    terminals.kill_all();
    assert!(start.elapsed() >= Duration::from_millis(100));
    for pid in pids {
        gone(pid).await;
    }
}

#[test]
fn kill_of_an_unknown_terminal_is_ignored() {
    kill(&mut Terminals::default(), ID);
}
