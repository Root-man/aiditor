//! The ACP recorder, the transcripts it recorded from the real adapters, and their replay through
//! the fake agent. The ignored `record_*` tests re-record them; see tests/transcripts/README.md.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use agent_client_protocol::schema::v1::{AgentRequest, Error, PromptRequest};
use aiditor_core::key::{Key, KeyCode, parse_keys};
use aiditor_core::{AgentId, AgentOp, Core, Effect, Msg};
use aiditor_services::agents::Agents;
use serde_json::Value;
use tokio::sync::mpsc::Receiver;
use tokio::time::timeout;

const ID: AgentId = AgentId(1);
const RECORDER: &str = env!("CARGO_BIN_EXE_aiditor-acp-record");
const FAKE: &str = env!("CARGO_BIN_EXE_aiditor-fake-agent");

fn transcripts() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/transcripts")
}

/// A session through the recorder in front of `command`, as `agent-start` opens one in `cwd`.
fn spawn(
    agents: &mut Agents,
    out: &Path,
    command: &str,
    cwd: &Path,
    env: &[(&str, &str)],
) -> Receiver<Msg> {
    let args = vec![out.display().to_string(), "--".into(), command.into()];
    (agents.run(AgentOp::Spawn {
        id: ID,
        command: RECORDER.into(),
        args,
        cwd: cwd.into(),
        env: (env.iter()).map(|&(k, v)| (k.into(), v.into())).collect(),
        session: None,
    }))
    .expect("spawn returns a receiver")
}

/// Answers the agent's requests as the core does, refusing each, until `done` matches.
async fn until(
    agents: &mut Agents,
    rx: &mut Receiver<Msg>,
    wait: Duration,
    done: fn(&Msg) -> bool,
) -> Msg {
    loop {
        let msg = timeout(wait, rx.recv()).await.expect("the agent answers");
        let msg = msg.expect("the agent is running");
        if let Msg::AgentStderr { line, .. } = &msg {
            eprintln!("agent: {line}");
        }
        if let Msg::AgentRequest { request, .. } = &msg {
            agents.run(AgentOp::Reply {
                id: ID,
                request: request.clone(),
                result: Err(Error::method_not_found()),
            });
        }
        if done(&msg) {
            return msg;
        }
    }
}

/// `text` with every email address replaced by `user@example.com`; Claude's adapter reports
/// the account's.
fn scrub_emails(text: &str) -> String {
    let local = |c: char| c.is_ascii_alphanumeric() || "._%+-".contains(c);
    let mut out = String::new();
    let mut rest = text;
    while let Some(at) = rest.find('@') {
        let start = rest[..at].trim_end_matches(local).len();
        let end = at
            + 1
            + rest[at + 1..]
                .find(|c: char| !local(c))
                .unwrap_or(rest.len() - at - 1);
        let domain = rest[at + 1..end].trim_end_matches('.');
        let end = at + 1 + domain.len();
        if start < at && domain.contains('.') {
            out.push_str(&rest[..start]);
            out.push_str("user@example.com");
        } else {
            out.push_str(&rest[..end]);
        }
        rest = &rest[end..];
    }
    out + rest
}

/// How many advertised commands a transcript keeps: the rest name the user's own skills and
/// their organization's apps.
const COMMANDS: usize = 3;

/// `line` with an `available_commands_update` cut to its first `COMMANDS` commands.
fn scrub_commands(line: &str) -> String {
    let mut record: Value = serde_json::from_str(line).expect(line);
    let update = &mut record["message"]["params"]["update"];
    match update["availableCommands"].as_array_mut() {
        Some(commands) if commands.len() > COMMANDS => commands.truncate(COMMANDS),
        _ => return line.into(),
    }
    record.to_string()
}

/// Every line of a transcript as `(from, message)`.
fn lines(text: &str) -> Vec<(String, Value)> {
    (text.lines())
        .map(|line| {
            let mut record: Value = serde_json::from_str(line).expect(line);
            let from = record["from"].as_str().expect(line).to_owned();
            assert!(from == "client" || from == "agent", "{line}");
            (from, record["message"].take())
        })
        .collect()
}

#[tokio::test(flavor = "multi_thread")]
async fn the_recorder_passes_a_session_through_and_logs_both_ways() {
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("out.jsonl");
    let mut agents = Agents::default();
    let mut rx = spawn(&mut agents, &out, FAKE, dir.path(), &[]);
    let started = until(&mut agents, &mut rx, Duration::from_secs(10), |m| {
        matches!(m, Msg::AgentStarted { .. })
    });
    let Msg::AgentStarted { result, .. } = started.await else {
        unreachable!()
    };
    let session = result.unwrap().session_id;
    let request = PromptRequest::new(session, vec!["hi".into()]);
    agents.run(AgentOp::Prompt { id: ID, request });
    let prompted = until(&mut agents, &mut rx, Duration::from_secs(10), |m| {
        matches!(m, Msg::AgentPrompted { .. })
    });
    let Msg::AgentPrompted { result, .. } = prompted.await else {
        unreachable!()
    };
    assert_eq!(format!("{:?}", result.unwrap()), "EndTurn");
    agents.kill_all();
    let lines = lines(&std::fs::read_to_string(&out).unwrap());
    let sent =
        |from: &str, method: &str| (lines.iter()).any(|(f, m)| f == from && m["method"] == method);
    assert!(sent("client", "initialize") && sent("client", "session/prompt"));
    let answers = lines
        .iter()
        .filter(|(f, m)| f == "agent" && m["result"].is_object());
    assert_eq!(
        answers.count(),
        3,
        "initialize, session/new, session/prompt"
    );
}

#[test]
fn the_recorder_rejects_bad_usage() {
    for args in [&["out"][..], &["out", "x", "cmd"], &["out", "--"]] {
        let output = std::process::Command::new(RECORDER)
            .args(args)
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(2), "{args:?}");
        assert!(String::from_utf8_lossy(&output.stderr).contains("usage"));
    }
}

#[test]
fn emails_are_scrubbed() {
    let text = r#"{"email":"a.b+c@mail.example.org","x":"@ref, me@host, end."}"#;
    let want = r#"{"email":"user@example.com","x":"@ref, me@host, end."}"#;
    assert_eq!(scrub_emails(text), want);
    assert_eq!(scrub_emails("x@y.z's"), "user@example.com's");
}

#[test]
fn committed_transcripts_parse_and_are_scrubbed() {
    let home = std::env::var("HOME").unwrap();
    let mut found = 0;
    for entry in std::fs::read_dir(transcripts()).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_none_or(|e| e != "jsonl") {
            continue;
        }
        let text = std::fs::read_to_string(&path).unwrap();
        let lines = lines(&text);
        assert!(!lines.is_empty(), "{}", path.display());
        // Streamed text can split a path across chunks, out of reach of a plain replace.
        let streamed: String = (lines.iter())
            .filter_map(|(_, m)| m["params"]["update"]["content"]["text"].as_str())
            .collect();
        for private in [&home[..], "/Users/", "/home/"] {
            assert!(!text.contains(private), "{} has {private}", path.display());
            assert!(
                !streamed.contains(private),
                "{} streams {private}",
                path.display()
            );
        }
        for line in text.lines() {
            assert_eq!(
                scrub_commands(line),
                line,
                "{} lists commands",
                path.display()
            );
        }
        let unscrubbed = text.replace("user@example.com", "");
        assert_eq!(
            scrub_emails(&unscrubbed),
            unscrubbed,
            "{} has an email",
            path.display()
        );
        found += 1;
    }
    assert_eq!(found, 2, "a Claude Code and a Codex transcript");
}

/// Every committed transcript's path.
fn committed() -> Vec<PathBuf> {
    let paths = std::fs::read_dir(transcripts())
        .unwrap()
        .map(|e| e.unwrap().path());
    let mut paths: Vec<_> = paths
        .filter(|p| p.extension().is_some_and(|e| e == "jsonl"))
        .collect();
    paths.sort();
    paths
}

/// A core running the agent service on the fake agent replaying a transcript, counting what
/// reaches it.
struct Replay {
    core: Core,
    agents: Agents,
    rx: Option<Receiver<Msg>>,
    stderr: Vec<String>,
    updates: usize,
    requests: usize,
    replies: usize,
}

impl Replay {
    fn new(transcript: &Path) -> Self {
        let mut replay = Replay {
            core: Core::default(),
            agents: Agents::default(),
            rx: None,
            stderr: Vec::new(),
            updates: 0,
            requests: 0,
            replies: 0,
        };
        let path = transcript.display().to_string();
        let settings = format!(
            "[agents.replay]\ncommand = {FAKE:?}\nenv = {{ AIDITOR_FAKE_AGENT_REPLAY = {path:?} }}"
        );
        replay.send(Msg::SettingsLoaded(Ok(Some(settings))));
        replay
    }

    fn send(&mut self, msg: Msg) {
        for effect in self.core.handle(Instant::now(), msg) {
            if let Effect::Agent(op) = effect {
                self.replies += matches!(op, AgentOp::Reply { .. }) as usize;
                if let Some(rx) = self.agents.run(op) {
                    self.rx = Some(rx);
                }
            }
        }
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

    /// Types `text` into the session buffer and sends it.
    fn prompt(&mut self, text: &str) {
        let text = text.into();
        self.send(Msg::Insert {
            text,
            replace: None,
        });
        self.keys("RET");
    }

    /// Fails with `what` and the agent's stderr, read until it exits, as that follows the stderr.
    async fn fail(&mut self, what: String) -> ! {
        while let Some(rx) = self.rx.as_mut()
            && let Ok(Some(msg)) = timeout(Duration::from_secs(5), rx.recv()).await
        {
            if let Msg::AgentStderr { line, .. } = msg {
                self.stderr.push(line);
            }
        }
        panic!("{what}; the agent's stderr:\n{}", self.stderr.join("\n"));
    }

    /// Feeds the agent's messages to the core until one matches `done`, which it returns.
    async fn pump(&mut self, done: fn(&Msg) -> bool) -> Msg {
        loop {
            let rx = self.rx.as_mut().expect("the agent was spawned");
            let msg = match timeout(Duration::from_secs(10), rx.recv()).await {
                Ok(Some(msg)) => msg,
                Ok(None) => self.fail("the agent's channel closed".into()).await,
                Err(_) => self.fail("no agent message within 10 s".into()).await,
            };
            match &msg {
                Msg::AgentStderr { line, .. } => self.stderr.push(line.clone()),
                Msg::AgentUpdate { update, .. } => {
                    self.updates += 1;
                    if format!("{update:?}").contains("{cwd}") {
                        (self.fail(format!("the replay left {{cwd}} in {update:?}"))).await
                    }
                }
                Msg::AgentRequest { .. } => self.requests += 1,
                Msg::AgentExited { code, signal, .. } => {
                    (self.fail(format!("the agent exited with {code} {signal:?}"))).await
                }
                _ => {}
            }
            self.send(msg.clone());
            if let Msg::AgentRequest { body, .. } = &msg
                && matches!(*body.0, AgentRequest::RequestPermissionRequest(_))
            {
                self.keys("C-c C-y");
            }
            if done(&msg) {
                return msg;
            }
        }
    }

    /// Opens the session and runs each recorded prompt, checking its stop reason.
    async fn run(&mut self, lines: &[(String, Value)]) {
        self.keys("M-x agent-start RET r e p l a y RET");
        let started = self.pump(|m| matches!(m, Msg::AgentStarted { .. })).await;
        if let Msg::AgentStarted {
            result: Err(err), ..
        } = started
        {
            self.fail(format!("the session didn't open: {err}")).await;
        }
        let prompts =
            (lines.iter()).filter(|(f, m)| f == "client" && m["method"] == "session/prompt");
        for (_, prompt) in prompts {
            let answer = (lines.iter()).find(|(f, m)| f == "agent" && m["id"] == prompt["id"]);
            let want = answer.expect("the prompt is answered").1["result"]["stopReason"].clone();
            self.prompt(
                prompt["params"]["prompt"][0]["text"]
                    .as_str()
                    .expect("a text prompt"),
            );
            let prompted = self.pump(|m| matches!(m, Msg::AgentPrompted { .. })).await;
            let Msg::AgentPrompted { result, .. } = prompted else {
                unreachable!()
            };
            match result.map(|reason| serde_json::to_value(reason).unwrap()) {
                Ok(reason) if reason == want => {}
                got => {
                    self.fail(format!("the turn ended with {got:?}, recorded {want}"))
                        .await
                }
            }
        }
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn committed_transcripts_replay_through_the_core() {
    let paths = committed();
    assert_eq!(paths.len(), 2, "a Claude Code and a Codex transcript");
    for path in paths {
        let lines = lines(&std::fs::read_to_string(&path).unwrap());
        let updates = (lines.iter())
            .filter(|(f, m)| f == "agent" && m["method"] == "session/update")
            .count();
        let requests = (lines.iter())
            .filter(|(f, m)| f == "agent" && m["method"].is_string() && !m["id"].is_null())
            .count();
        let mut replay = Replay::new(&path);
        replay.run(&lines).await;
        let name = path.display();
        assert_eq!(replay.updates, updates, "{name}: typed updates");
        assert!(requests > 0, "{name} has a permission request");
        assert_eq!(replay.requests, requests, "{name}: agent requests");
        // The replay only goes on once the client answers, so the turns ending prove it arrived.
        assert_eq!(replay.replies, requests, "{name}: answered requests");
        let asks = (lines.iter())
            .filter(|(f, m)| f == "agent" && m["method"] == "session/request_permission")
            .count();
        let text = replay.core.text();
        assert_eq!(text.matches("\nChose ").count(), asks, "{name}: {text}");
        replay.agents.kill_all();
    }
}

#[test]
fn a_bad_transcript_exits_2_and_closed_stdin_exits_0() {
    let dir = tempfile::tempdir().unwrap();
    let garbage = dir.path().join("garbage.jsonl");
    std::fs::write(&garbage, "not json\n").unwrap();
    let missing = dir.path().join("missing.jsonl");
    let run = |path: &Path| {
        let output = std::process::Command::new(FAKE)
            .env("AIDITOR_FAKE_AGENT_REPLAY", path)
            .stdin(std::process::Stdio::null())
            .output()
            .unwrap();
        (
            output.status.code(),
            String::from_utf8_lossy(&output.stderr).into_owned(),
        )
    };
    for path in [&garbage, &missing] {
        let (code, stderr) = run(path);
        assert_eq!(code, Some(2), "{stderr}");
        assert!(stderr.contains(&path.display().to_string()), "{stderr}");
    }
    let (code, stderr) = run(&transcripts().join("claude.jsonl"));
    assert_eq!(code, Some(0), "{stderr}");
}

#[tokio::test(flavor = "multi_thread")]
#[should_panic(expected = "unexpected client message: ")]
async fn an_unexpected_client_message_fails_the_replay() {
    let path = transcripts().join("claude.jsonl");
    let mut replay = Replay::new(&path);
    replay.keys("M-x agent-start RET r e p l a y RET");
    replay.pump(|m| matches!(m, Msg::AgentStarted { .. })).await;
    // The core cancels before the replay reads on; the transcript expects the next prompt.
    replay.prompt("hi");
    replay.keys("C-c C-c");
    loop {
        replay.pump(|_| false).await;
    }
}

/// Runs `prompts` with `command` and `env` in a scratch project holding `notes.md`, through the
/// recorder, then writes the scrubbed transcript to `tests/transcripts/<name>.jsonl`.
async fn record(name: &str, command: &str, env: &[(&str, &str)], prompts: &[&str]) {
    let (project, scratch) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let cwd = project.path().canonicalize().unwrap();
    std::fs::write(cwd.join("notes.md"), "# Notes\n\nFirst note.\n").unwrap();
    let out = scratch.path().join("raw.jsonl");
    let mut agents = Agents::default();
    let mut rx = spawn(&mut agents, &out, command, &cwd, env);
    let started = until(&mut agents, &mut rx, Duration::from_secs(60), |m| {
        matches!(m, Msg::AgentStarted { .. })
    });
    let Msg::AgentStarted { result, .. } = started.await else {
        unreachable!()
    };
    let session = result.expect("the session opens").session_id;
    for prompt in prompts {
        let request = PromptRequest::new(session.clone(), vec![(*prompt).into()]);
        agents.run(AgentOp::Prompt { id: ID, request });
        let turn = until(&mut agents, &mut rx, Duration::from_secs(300), |m| {
            matches!(m, Msg::AgentPrompted { .. })
        });
        let Msg::AgentPrompted { result, .. } = turn.await else {
            unreachable!()
        };
        eprintln!("{prompt:?}: {result:?}");
    }
    agents.kill_all();
    let mut text = std::fs::read_to_string(&out).unwrap();
    let home = std::env::var("HOME").unwrap();
    // The longest first, so `{cwd}` wins over `{home}`; macOS temp dirs also show as /private/var.
    let as_given = project.path().display().to_string();
    for (from, to) in [(&cwd.display().to_string(), "{cwd}"), (&as_given, "{cwd}")] {
        text = text.replace(from, to);
    }
    let text = scrub_emails(&text.replace(&home, "{home}"));
    let text: String = (text.lines())
        .map(|line| scrub_commands(line) + "\n")
        .collect();
    std::fs::write(transcripts().join(format!("{name}.jsonl")), text).unwrap();
}

const PROMPTS: [&str; 3] = [
    "Reply with a short markdown list of two fruits, the second in bold, and nothing else.",
    "Read notes.md and tell me its first line.",
    "With your file editing tool, append the line `Second note.` to notes.md.",
];

#[tokio::test(flavor = "multi_thread")]
#[ignore = "records a real Claude Code session; needs claude-agent-acp and a login"]
async fn record_claude() {
    // Its edit asks, so it is the permission prompt; a project can't set `acceptEdits` untrusted.
    let plan = "Enter plan mode, then reply with only the word ready.";
    record(
        "claude",
        "claude-agent-acp",
        &[],
        &[&PROMPTS[..], &[plan]].concat(),
    )
    .await;
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "records a real Codex session; needs codex-acp and a login"]
async fn record_codex() {
    // Its default `agent` mode lets a reviewer model approve; `workspace-write` asks the client
    // before network access. Codex can't change its own mode, so it records no mode change.
    let env = [("INITIAL_AGENT_MODE", "workspace-write")];
    let ask = "Run the shell command `curl -sI https://example.com` and tell me the status line.";
    record("codex", "codex-acp", &env, &[&PROMPTS[..], &[ask]].concat()).await;
}
