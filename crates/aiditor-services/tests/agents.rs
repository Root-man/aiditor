use std::path::Path;
use std::time::{Duration, Instant};

use agent_client_protocol::schema::v1::{
    PromptRequest, SessionConfigKind, SetSessionConfigOptionRequest, StopReason,
};
use aiditor_core::key::{Key, KeyCode, parse_keys};
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
    // The session steps run once `session/new` is answered, so their update can come first.
    let update = loop {
        match next(&mut rx).await {
            Some(Msg::AgentStderr { .. } | Msg::AgentStarted { result: Ok(_), .. }) => {}
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

/// Starts the fake agent on `script`, ready for prompts.
async fn ready(dir: &Path, script_text: &str) -> (Agents, Receiver<Msg>) {
    let (key, path) = script(dir, script_text);
    let mut agents = Agents::default();
    let mut rx = spawn(&mut agents, AGENT, dir, &[(key, &path)]);
    started(&mut rx).await.1.unwrap();
    (agents, rx)
}

fn prompt(text: &str) -> AgentOp {
    let request = PromptRequest::new("fake-session", vec![text.into()]);
    AgentOp::Prompt { id: ID, request }
}

/// The messages up to the prompt's answer, and its result.
async fn turn(rx: &mut Receiver<Msg>) -> (Vec<Msg>, Result<StopReason, String>) {
    let mut msgs = Vec::new();
    loop {
        match next(rx).await.expect("the agent is running") {
            Msg::AgentPrompted { id, result } => {
                assert_eq!(id, ID);
                return (msgs, result);
            }
            msg => msgs.push(msg),
        }
    }
}

/// A fake agent script step sending a text message chunk.
fn chunk(text: &str) -> String {
    format!(
        r#"{{"update": {{"sessionUpdate": "agent_message_chunk",
            "content": {{"type": "text", "text": "{text}"}}}}}}"#
    )
}

#[tokio::test(flavor = "multi_thread")]
async fn a_prompt_streams_its_updates_before_its_answer() {
    let dir = tempfile::tempdir().unwrap();
    let (a, b) = (chunk("a"), chunk("b"));
    let script = format!(r#"{{"prompts": [[{a}, {b}, {{"stop": "refusal"}}]]}}"#);
    let (mut agents, mut rx) = ready(dir.path(), &script).await;
    assert!(agents.run(prompt("hello")).is_none());
    let (msgs, result) = turn(&mut rx).await;
    assert_eq!(result, Ok(StopReason::Refusal));
    let updates = (msgs.iter()).filter(|m| matches!(m, Msg::AgentUpdate { .. }));
    assert_eq!(updates.count(), 2, "{msgs:?}");
    agents.kill_all();
}

#[tokio::test(flavor = "multi_thread")]
async fn setting_a_config_option_returns_the_answer() {
    let dir = tempfile::tempdir().unwrap();
    let (mut agents, mut rx) = ready(dir.path(), "{}").await;
    for (value, ok) in [("plan", true), ("nope", false)] {
        let request = SetSessionConfigOptionRequest::new("fake-session", "mode", value);
        assert!(
            agents
                .run(AgentOp::SetConfigOption { id: ID, request })
                .is_none()
        );
        let result = loop {
            match next(&mut rx).await {
                Some(Msg::AgentConfigured { id, result }) => break (assert_eq!(id, ID), result).1,
                Some(Msg::AgentUpdate { .. }) => {}
                msg => panic!("expected configured, got {msg:?}"),
            }
        };
        assert_eq!(result.is_ok(), ok, "{result:?}");
        if let Ok(options) = result {
            let mode = options.iter().find(|o| o.id.0.as_ref() == "mode").unwrap();
            let SessionConfigKind::Select(select) = &mode.kind else {
                panic!("a select: {mode:?}");
            };
            assert_eq!(select.current_value.0.as_ref(), "plan");
        }
    }
    agents.kill_all();
}

#[tokio::test(flavor = "multi_thread")]
async fn a_cancel_ends_the_running_turn() {
    let dir = tempfile::tempdir().unwrap();
    let (mut agents, mut rx) = ready(dir.path(), r#"{"prompts": [[{"sleep": 10000}]]}"#).await;
    agents.run(prompt("hello"));
    let session = "fake-session".into();
    agents.run(AgentOp::Cancel { id: ID, session });
    assert_eq!(turn(&mut rx).await.1, Ok(StopReason::Cancelled));
    // Unknown agents are ignored.
    let session = "fake-session".into();
    agents.run(AgentOp::Cancel {
        id: AgentId(9),
        session,
    });
    let AgentOp::Prompt { request, .. } = prompt("x") else {
        unreachable!()
    };
    let unknown = AgentOp::Prompt {
        id: AgentId(9),
        request,
    };
    assert!(agents.run(unknown).is_none());
    agents.kill_all();
}

/// A core running the real agent service on the fake agent, driven by keys.
struct Live {
    core: Core,
    agents: Agents,
    rx: Option<Receiver<Msg>>,
    pid: Option<libc::pid_t>,
}

impl Live {
    fn new(dir: &Path, script_text: &str) -> Self {
        let (key, path) = script(dir, script_text);
        let mut live = Live {
            core: Core::default(),
            agents: Agents::default(),
            rx: None,
            pid: None,
        };
        let settings = format!("[agents.fake]\ncommand = {AGENT:?}\nenv = {{ {key} = {path:?} }}");
        live.send(Msg::SettingsLoaded(Ok(Some(settings))));
        live
    }

    fn run(&mut self, effects: aiditor_core::Effects) {
        for effect in effects {
            if let Effect::Agent(op) = effect
                && let Some(rx) = self.agents.run(op)
            {
                self.rx = Some(rx);
            }
        }
    }

    fn send(&mut self, msg: Msg) {
        let effects = self.core.handle(Instant::now(), msg);
        self.run(effects);
    }

    fn keys(&mut self, script: &str) {
        for key in parse_keys(script).unwrap() {
            self.send(match key.code {
                KeyCode::Char(c) if key == Key::plain(key.code) => Msg::Insert {
                    text: c.into(),
                    replace: None,
                },
                _ => Msg::Key(key),
            });
        }
    }

    /// Runs the command `name` with `strings`, as `M-x` would after its prompts.
    fn call(&mut self, name: &str, strings: &[&str]) {
        let mut ctx = aiditor_core::Ctx {
            core: &mut self.core,
            now: Instant::now(),
            effects: Vec::new(),
        };
        let strings = strings.iter().map(|s| (*s).into()).collect();
        ctx.call(
            name,
            aiditor_core::command::Args {
                strings,
                ..Default::default()
            },
        );
        let effects = ctx.effects;
        self.run(effects);
        // The core reconciles killed buffers' sessions as it handles a message.
        self.send(Msg::Tick);
    }

    /// Feeds the agent's messages to the core until one matches `done`, which is fed too, or,
    /// with `None`, until the agent's channel closes.
    async fn pump(&mut self, done: Option<fn(&Msg) -> bool>) {
        loop {
            let rx = self.rx.as_mut().expect("an agent was spawned");
            let Some(msg) = next(rx).await else {
                assert!(done.is_none(), "the agent's channel closed");
                return;
            };
            if let Msg::AgentStderr { line, .. } = &msg
                && let Some(pid) = line.strip_prefix("pid=")
            {
                self.pid = Some(pid.parse().unwrap());
            }
            let matched = done.is_some_and(|done| done(&msg));
            self.send(msg);
            if matched {
                return;
            }
        }
    }

    async fn start(&mut self) {
        self.keys("M-x agent-start RET f a k e RET");
        (self.pump(Some(|m| matches!(m, Msg::AgentStarted { .. })))).await;
        assert!(self.core.current().name.starts_with("*fake*<"));
    }

    async fn turn(&mut self) {
        (self.pump(Some(|m| matches!(m, Msg::AgentPrompted { .. })))).await;
    }

    /// Waits for the agent to exit and checks its process is gone.
    async fn gone(&mut self) {
        self.pump(None).await;
        assert!(gone(self.pid.expect("the agent reported its pid")).await);
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn the_session_buffer_runs_turns_and_its_kill_stops_the_agent() {
    let dir = tempfile::tempdir().unwrap();
    let (hi, there) = (chunk("Hi"), chunk(" there"));
    let script = format!(r#"{{"prompts": [[{hi}, {there}], [{{"sleep": 10000}}]]}}"#);
    let mut live = Live::new(dir.path(), &script);
    live.start().await;
    live.keys("h e l l o RET");
    live.turn().await;
    assert_eq!(live.core.text(), "> hello\n\nHi there\n\n> ");
    live.keys("w a i t RET C-c C-c");
    live.turn().await;
    assert_eq!(
        live.core.text(),
        "> hello\n\nHi there\n\n> wait\n\nTurn ended: cancelled\n\n> "
    );
    let name = live.core.current().name.clone();
    live.call("kill-buffer", &[&name, "yes"]);
    live.gone().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn killing_the_workspace_stops_its_agent() {
    let dir = tempfile::tempdir().unwrap();
    let mut live = Live::new(dir.path(), "{}");
    live.keys("C-x t 2 RET f i x RET");
    live.start().await;
    live.keys("C-x t 0");
    live.gone().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_session_starts_in_its_default_mode() {
    let dir = tempfile::tempdir().unwrap();
    let mut live = Live::new(dir.path(), "{}");
    let (key, path) = script(dir.path(), "{}");
    let settings = format!(
        "[agents.fake]\ncommand = {AGENT:?}\ndefault_mode = \"plan\"\nenv = {{ {key} = {path:?} }}"
    );
    live.send(Msg::SettingsLoaded(Ok(Some(settings))));
    live.start().await;
    (live.pump(Some(|m| matches!(m, Msg::AgentConfigured { .. })))).await;
    let mode_line = live.core.frame().windows[0].display.mode_line.clone();
    assert!(mode_line.contains(" [plan]"), "{mode_line}");
    live.call("kill-buffer", &[&live.core.current().name.clone(), "yes"]);
    live.gone().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn s_tab_cycles_the_mode() {
    let dir = tempfile::tempdir().unwrap();
    let mut live = Live::new(dir.path(), "{}");
    live.start().await;
    for mode in ["plan", "auto"] {
        live.keys("S-TAB");
        (live.pump(Some(|m| matches!(m, Msg::AgentConfigured { .. })))).await;
        let mode_line = live.core.frame().windows[0].display.mode_line.clone();
        assert!(mode_line.contains(&format!(" [{mode}]")), "{mode_line}");
    }
    live.call("kill-buffer", &[&live.core.current().name.clone(), "yes"]);
    live.gone().await;
}
