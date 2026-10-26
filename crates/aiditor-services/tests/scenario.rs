//! The whole core on the real agent, file and watcher services: the fake agent writes a file
//! the project has open while it answers a prompt.

use std::fs;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use aiditor_core::key::{Key, KeyCode, parse_keys};
use aiditor_core::{Core, Effect, Msg};
use aiditor_services::agents::Agents;
use aiditor_services::{files, projects, watcher::Watcher};
use tokio::sync::mpsc::{Receiver, UnboundedReceiver, unbounded_channel};

const AGENT: &str = env!("CARGO_BIN_EXE_aiditor-fake-agent");
const TIMEOUT: Duration = Duration::from_secs(10);

struct App {
    core: Core,
    agents: Agents,
    agent: Option<Receiver<Msg>>,
    pid: Option<libc::pid_t>,
    started: bool,
    prompted: bool,
    exited: bool,
    watcher: Watcher,
    reports: UnboundedReceiver<Msg>,
    root: PathBuf,
    _dir: tempfile::TempDir,
}

impl App {
    /// A project `scenario` holding `a.txt`, with the fake agent registered as `fake` on `script`.
    fn new(script: &str) -> Self {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap().join("repo");
        fs::create_dir_all(root.join(".git")).unwrap();
        fs::write(root.join("a.txt"), "before\n").unwrap();
        let script_path = dir.path().join("script.json");
        fs::write(&script_path, script).unwrap();
        let project = projects::write(
            &dir.path().join("projects/scenario.toml"),
            None,
            format!("name = 'scenario'\n[primary]\npath = {root:?}\n"),
        )
        .unwrap();
        let (tx, reports) = unbounded_channel();
        let mut app = Self {
            core: Core::default(),
            agents: Agents::default(),
            agent: None,
            pid: None,
            started: false,
            prompted: false,
            exited: false,
            watcher: Watcher::spawn(move |msg| _ = tx.send(msg)),
            reports,
            root,
            _dir: dir,
        };
        let settings = format!(
            "[agents.fake]\ncommand = {AGENT:?}\nenv = {{ AIDITOR_FAKE_AGENT = {script_path:?} }}"
        );
        app.send(Msg::SettingsLoaded(Ok(Some(settings))));
        app.send(Msg::ProjectsLoaded {
            dir: None,
            projects: vec![Ok(project)],
        });
        app.core.launch(&app.root, &app.root);
        app
    }

    fn send(&mut self, msg: Msg) {
        for effect in self.core.handle(Instant::now(), msg) {
            match effect {
                Effect::Agent(op) => {
                    if let Some(rx) = self.agents.run(op) {
                        self.agent = Some(rx);
                    }
                }
                Effect::File(op) => {
                    let reply = files::run(op);
                    self.send(reply);
                }
                Effect::Watch(set) => self.watcher.set(set),
                Effect::Terminal(_) | Effect::Recovery(_) | Effect::SetClipboard(_) => {}
                Effect::Grep(_)
                | Effect::LoadConfiguration { .. }
                | Effect::CaptureEnvironment { .. }
                | Effect::Quit => panic!("unexpected effect in the scenario"),
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

    fn open(&mut self, name: &str) {
        let path = self.root.join(name);
        self.core.open(&path, files::read(&path).unwrap());
        self.send(Msg::Tick);
    }

    /// The next message from the watcher or the agent within `wait`, recording the agent's
    /// pid, start, answer and exit.
    async fn next(&mut self, wait: Duration) -> Option<Msg> {
        let msg = tokio::time::timeout(wait, async {
            loop {
                let agent = async {
                    match self.agent.as_mut() {
                        Some(rx) => rx.recv().await,
                        None => std::future::pending().await,
                    }
                };
                tokio::select! {
                    Some(msg) = self.reports.recv() => return msg,
                    msg = agent => match msg {
                        Some(msg) => return msg,
                        None => self.agent = None,
                    },
                }
            }
        })
        .await
        .ok()?;
        match &msg {
            Msg::AgentStderr { line, .. } => {
                if let Some(pid) = line.strip_prefix("pid=") {
                    self.pid = Some(pid.parse().unwrap());
                }
            }
            Msg::AgentStarted { .. } => self.started = true,
            Msg::AgentPrompted { .. } => self.prompted = true,
            Msg::AgentExited { .. } => self.exited = true,
            _ => {}
        }
        Some(msg)
    }

    /// Feeds the watcher's and the agent's messages to the core until `done` holds.
    async fn until(&mut self, what: &str, done: impl Fn(&App) -> bool) {
        let deadline = Instant::now() + TIMEOUT;
        while !done(self) {
            let wait = deadline.saturating_duration_since(Instant::now());
            let Some(msg) = self.next(wait).await else {
                panic!(
                    "timed out waiting for {what}; text = {:?}",
                    self.core.text()
                );
            };
            self.send(msg);
        }
    }

    /// Rewrites `name` until the watcher reports it, as `file_sync.rs` does.
    async fn ready(&mut self, name: &str) {
        let path = self.root.join(name);
        let bytes = fs::read(&path).unwrap();
        let deadline = Instant::now() + TIMEOUT;
        loop {
            assert!(
                Instant::now() < deadline,
                "watcher never ready for {path:?}"
            );
            fs::write(&path, &bytes).unwrap();
            while let Some(msg) = self.next(Duration::from_millis(200)).await {
                let ready = matches!(&msg, Msg::FilesChanged { paths, .. }
                    if paths.iter().any(|(p, _)| path.starts_with(p)));
                self.send(msg);
                if ready {
                    return;
                }
            }
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
async fn an_agent_reply_and_write_reach_the_editor_and_its_workspace_kill_stops_it() {
    let (done, edited) = (chunk("Done"), chunk(", edited a.txt"));
    let write = r#"{"write": {"path": "a.txt", "text": "after\n"}}"#;
    let mut app = App::new(&format!(r#"{{"prompts": [[{done}, {write}, {edited}]]}}"#));
    app.keys("C-x t 2 RET agent RET");
    app.open("a.txt");
    app.ready("a.txt").await;
    let file = app.core.current().name.clone();

    app.keys("M-x agent-start RET fake RET");
    app.until("the session", |app| app.started).await;
    assert_eq!(app.core.current().name, "*fake*<agent>");
    app.keys("edit SPC it RET");
    app.until("the turn", |app| app.prompted).await;
    assert_eq!(app.core.text(), "> edit it\n\nDone, edited a.txt\n\n> ");

    app.keys(&format!("C-x b {file} RET"));
    app.until("the open buffer to revert", |app| {
        app.core.text() == "after\n"
    })
    .await;
    assert!(!app.core.current().modified());

    app.keys("C-x t 0");
    app.until("the agent to exit", |app| app.exited).await;
    let pid = app.pid.expect("the agent reported its pid");
    let deadline = Instant::now() + Duration::from_secs(5);
    while unsafe { libc::kill(pid, 0) } == 0 {
        assert!(Instant::now() < deadline, "agent {pid} is still running");
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}
