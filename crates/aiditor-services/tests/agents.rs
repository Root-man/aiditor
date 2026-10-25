use std::path::Path;
use std::time::{Duration, Instant};

use aiditor_core::{AgentId, AgentOp, Core, Effect, Msg};
use aiditor_services::agents::Agents;
use tokio::sync::mpsc::Receiver;
use tokio::time::timeout;

const ID: AgentId = AgentId(3);
const AGENT: &str = env!("CARGO_BIN_EXE_aiditor-fake-agent");

/// Writes `script` into `dir` for the fake agent; returns the variable naming it.
fn script(dir: &Path, script: &str) -> (&'static str, String) {
    let path = dir.join("script.json");
    std::fs::write(&path, script).unwrap();
    ("AIDITOR_FAKE_AGENT", path.display().to_string())
}

fn spawn(agents: &mut Agents, command: &str, cwd: &Path, env: &[(&str, &str)]) -> Receiver<Msg> {
    let env = (env.iter()).map(|&(k, v)| (k.into(), v.into())).collect();
    (agents.run(AgentOp::Spawn {
        id: ID,
        command: command.into(),
        args: Vec::new(),
        cwd: cwd.into(),
        env,
    }))
    .expect("spawn returns a receiver")
}

async fn next(rx: &mut Receiver<Msg>) -> Option<Msg> {
    timeout(Duration::from_secs(10), rx.recv())
        .await
        .expect("an agent message within 10 s")
}

/// The stderr lines until the started message, and that message's result.
async fn started(rx: &mut Receiver<Msg>) -> (Vec<String>, Result<String, String>) {
    let mut lines = Vec::new();
    loop {
        match next(rx).await {
            Some(Msg::AgentStderr { line, .. }) => lines.push(line),
            Some(Msg::AgentStarted { id, result }) => {
                assert_eq!(id, ID);
                return (lines, result.map(|session| format!("{session:?}")));
            }
            msg => panic!("expected stderr or started, got {msg:?}"),
        }
    }
}

fn pid(lines: &[String]) -> libc::pid_t {
    let line = lines.iter().find_map(|l| l.strip_prefix("pid="));
    line.expect("the agent reports its pid").parse().unwrap()
}

/// Waits up to 5 s for `pid` to be gone, reaped by the service.
async fn gone(pid: libc::pid_t) -> bool {
    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline {
        if unsafe { libc::kill(pid, 0) } != 0 {
            return true;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    false
}

/// Every message until the channel closes.
async fn rest(rx: &mut Receiver<Msg>) -> Vec<Msg> {
    let mut msgs = Vec::new();
    while let Some(msg) = next(rx).await {
        msgs.push(msg);
    }
    msgs
}

#[tokio::test(flavor = "multi_thread")]
async fn a_session_opens_in_cwd_without_fs_or_terminal() {
    let dir = tempfile::tempdir().unwrap();
    let mut agents = Agents::default();
    let mut rx = spawn(&mut agents, AGENT, dir.path(), &[]);
    let (lines, result) = started(&mut rx).await;
    let session = result.unwrap();
    assert!(session.contains("fake-session"), "{session}");
    assert!(
        session.contains("auto") && session.contains("plan"),
        "{session}"
    );
    let cwd = format!("cwd={}", dir.path().display());
    assert!(lines.contains(&cwd), "{lines:?}");
    let caps = "fs.readTextFile=false fs.writeTextFile=false terminal=false";
    assert!(lines.iter().any(|l| l == caps), "{lines:?}");
    agents.kill_all();
}

#[tokio::test(flavor = "multi_thread")]
async fn later_variables_win() {
    let dir = tempfile::tempdir().unwrap();
    let mut agents = Agents::default();
    let mut rx = spawn(&mut agents, AGENT, dir.path(), &[("A", "1"), ("A", "2")]);
    let (lines, result) = started(&mut rx).await;
    result.unwrap();
    assert!(lines.iter().any(|l| l == "A=2"), "{lines:?}");
    agents.kill_all();
}

#[tokio::test(flavor = "multi_thread")]
async fn the_command_is_found_on_the_given_path() {
    let (dir, name) = AGENT.rsplit_once('/').unwrap();
    let mut agents = Agents::default();
    let mut rx = spawn(&mut agents, name, Path::new("/"), &[("PATH", dir)]);
    started(&mut rx).await.1.unwrap();
    agents.kill_all();
}

#[tokio::test(flavor = "multi_thread")]
async fn a_missing_program_fails_to_start() {
    let mut agents = Agents::default();
    let mut rx = spawn(&mut agents, "/nonexistent/agent", Path::new("/"), &[]);
    let (_, result) = started(&mut rx).await;
    assert!(result.unwrap_err().contains("/nonexistent/agent"));
    assert!(rest(&mut rx).await.is_empty());
}

#[tokio::test(flavor = "multi_thread")]
async fn a_missing_cwd_fails_to_start() {
    let mut agents = Agents::default();
    let mut rx = spawn(&mut agents, AGENT, Path::new("/nonexistent/dir"), &[]);
    let (_, result) = started(&mut rx).await;
    assert!(result.unwrap_err().contains("not a directory"));
    assert!(rest(&mut rx).await.is_empty());
}

#[tokio::test(flavor = "multi_thread")]
async fn an_agent_exiting_before_answering_fails_then_exits() {
    let mut agents = Agents::default();
    let mut rx = spawn(&mut agents, "/usr/bin/true", Path::new("/"), &[]);
    let (_, result) = started(&mut rx).await;
    result.unwrap_err();
    let msgs = rest(&mut rx).await;
    assert!(
        matches!(
            &msgs[..],
            [Msg::AgentExited {
                code: 0,
                signal: None,
                ..
            }]
        ),
        "{msgs:?}"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn stderr_then_exit() {
    let dir = tempfile::tempdir().unwrap();
    let (key, path) = script(
        dir.path(),
        r#"{"session": [{"stderr": "first"}, {"stderr": "second"}, {"exit": 3}]}"#,
    );
    let mut agents = Agents::default();
    let mut rx = spawn(&mut agents, AGENT, Path::new("/"), &[(key, &path)]);
    let msgs = rest(&mut rx).await;
    let lines: Vec<_> = (msgs.iter())
        .filter_map(|msg| match msg {
            Msg::AgentStderr { line, .. } => Some(line.as_str()),
            _ => None,
        })
        .collect();
    assert!(lines.ends_with(&["first", "second"]), "{lines:?}");
    assert!(
        matches!(
            msgs.last(),
            Some(Msg::AgentExited {
                code: 3,
                signal: None,
                ..
            })
        ),
        "{msgs:?}"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn the_core_rejects_agent_requests() {
    let dir = tempfile::tempdir().unwrap();
    let ask = r#"{"toolCall": {"toolCallId": "call-1"},
        "options": [{"optionId": "allow", "name": "Allow", "kind": "allow_once"}]}"#;
    let (key, path) = script(
        dir.path(),
        &format!(r#"{{"session": [{{"permission": {ask}}}]}}"#),
    );
    let mut agents = Agents::default();
    let mut rx = spawn(&mut agents, AGENT, Path::new("/"), &[(key, &path)]);
    let mut core = Core::default();
    loop {
        let msg = next(&mut rx).await.expect("the agent is running");
        if let Msg::AgentStderr { line, .. } = &msg
            && line.starts_with("permission ")
        {
            assert_eq!(line, "permission error: Method not found");
            break;
        }
        for effect in core.handle(Instant::now(), msg) {
            if let Effect::Agent(op) = effect {
                assert!(agents.run(op).is_none());
            }
        }
    }
    agents.kill_all();
}

#[tokio::test(flavor = "multi_thread")]
async fn session_updates_reach_the_core() {
    let dir = tempfile::tempdir().unwrap();
    let update = r#"{"sessionUpdate": "agent_message_chunk",
        "content": {"type": "text", "text": "hi"}}"#;
    let (key, path) = script(
        dir.path(),
        &format!(r#"{{"session": [{{"update": {update}}}]}}"#),
    );
    let mut agents = Agents::default();
    let mut rx = spawn(&mut agents, AGENT, Path::new("/"), &[(key, &path)]);
    started(&mut rx).await.1.unwrap();
    let update = loop {
        match next(&mut rx).await {
            Some(Msg::AgentStderr { .. }) => {}
            Some(Msg::AgentUpdate { id, update }) => break (id, update),
            msg => panic!("expected an update, got {msg:?}"),
        }
    };
    assert_eq!(update.0, ID);
    let update = format!("{:?}", update.1);
    assert!(
        update.contains("fake-session") && update.contains("AgentMessageChunk"),
        "{update}"
    );
    assert!(update.contains("\"hi\""), "{update}");
    agents.kill_all();
}

#[tokio::test(flavor = "multi_thread")]
async fn a_killed_agent_exits_and_is_gone() {
    let mut agents = Agents::default();
    let mut rx = spawn(&mut agents, AGENT, Path::new("/"), &[]);
    let (lines, result) = started(&mut rx).await;
    result.unwrap();
    let start = Instant::now();
    assert!(agents.run(AgentOp::Kill { id: ID }).is_none());
    assert!(
        start.elapsed() < Duration::from_millis(50),
        "kill returns at once"
    );
    let msgs = rest(&mut rx).await;
    assert!(
        matches!(
            msgs.last(),
            Some(Msg::AgentExited {
                signal: Some(_),
                ..
            })
        ),
        "{msgs:?}"
    );
    assert!(gone(pid(&lines)).await);
    // Unknown now: ignored.
    assert!(agents.run(AgentOp::Kill { id: ID }).is_none());
}

#[tokio::test(flavor = "multi_thread")]
async fn quitting_kills_every_agent() {
    let dir = tempfile::tempdir().unwrap();
    let (key, path) = script(
        dir.path(),
        r#"{"session": ["ignoreSigterm", {"stderr": "ready"}]}"#,
    );
    let mut agents = Agents::default();
    let mut running = Vec::new();
    for (n, script) in [(1, None), (2, Some(path.as_str()))] {
        let mut rx = (agents.run(AgentOp::Spawn {
            id: AgentId(n),
            command: AGENT.into(),
            args: Vec::new(),
            cwd: "/".into(),
            env: script.map(|s| (key.into(), s.into())).into_iter().collect(),
        }))
        .unwrap();
        // The trapping agent is killed only once SIGTERM is ignored.
        let mut lines = Vec::new();
        let mut started = false;
        while !started || (script.is_some() && !lines.iter().any(|l| l == "ready")) {
            match next(&mut rx).await {
                Some(Msg::AgentStderr { line, .. }) => lines.push(line),
                Some(Msg::AgentStarted { result: Ok(_), .. }) => started = true,
                msg => panic!("expected stderr or started, got {msg:?}"),
            }
        }
        running.push((pid(&lines), rx));
    }
    let start = Instant::now();
    agents.kill_all();
    assert!(start.elapsed() >= Duration::from_millis(100));
    for (pid, mut rx) in running {
        let msgs = rest(&mut rx).await;
        assert!(
            matches!(msgs.last(), Some(Msg::AgentExited { .. })),
            "{msgs:?}"
        );
        assert!(gone(pid).await);
    }
}
