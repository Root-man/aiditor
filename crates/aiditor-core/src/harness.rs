//! Headless core driven by key scripts and a fake clock, for tests.

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use crate::{
    Core, Ctx, Effect, Effects, FileOp, GrepOp, Hit, Msg, RecoveryOp, WatchSet,
    buffer::{BufferId, hash},
    command::{ArgSpec, Args, Command},
    display::{FrameDisplay, WindowDisplay},
    key::{Key, KeyCode, parse_keys},
    timer::TimerId,
};

type ReadReply = Result<Option<(Vec<u8>, u64)>, String>;

pub struct Harness {
    pub core: Core,
    pub dir: PathBuf,
    /// The fake clock; only `advance` moves it.
    pub now: Instant,
    /// The fake system clipboard.
    pub clipboard: Option<String>,
    /// Whether the core asked the app to quit.
    pub quit: bool,
    /// The recovery files: each buffer's auto-saved name, path and text.
    pub recovery: HashMap<BufferId, (String, Option<PathBuf>, String)>,
    /// Earlier sessions' recovery files deleted, in order.
    pub deleted: Vec<PathBuf>,
    /// Each grep search's pattern, and `None` for each cancel, in order.
    pub greps: Vec<Option<String>>,
    pub reads: HashMap<PathBuf, ReadReply>,
    pub listings: HashMap<PathBuf, Result<Vec<crate::FileEntry>, String>>,
    pub indexes: HashMap<PathBuf, Result<Vec<PathBuf>, String>>,
    pub directories: HashMap<PathBuf, Vec<PathBuf>>,
    pub checks: HashMap<PathBuf, Option<u64>>,
    /// Each file's answer to `FileOp::Head`; none is outside a repository.
    pub heads: HashMap<PathBuf, crate::Head>,
    pub saves: std::collections::VecDeque<Result<u64, String>>,
    pub writes: Vec<(PathBuf, String)>,
    pub reverts: HashMap<BufferId, Result<crate::buffer::Revert, String>>,
    pub diffs: HashMap<PathBuf, Result<String, String>>,
    pub hits: std::collections::VecDeque<Result<Vec<Hit>, String>>,
    /// What the core last asked the watcher to watch.
    pub watched: WatchSet,
    pub collect_effects: bool,
    pub effects: Effects,
    /// The terminal work the core asked for, in order.
    pub terminal_ops: Vec<crate::TerminalOp>,
    /// The agent work the core asked for, in order.
    pub agent_ops: Vec<crate::AgentOp>,
}

impl Harness {
    /// A core with prepared read replies for (relative path, contents), without disk I/O.
    pub fn new(files: &[(&str, &str)]) -> Self {
        let dir = PathBuf::from("/aiditor-test");
        let mut core = Core::default();
        // Named the same whatever `$HOME` is, so scratch names, titles and snapshots don't vary.
        core.rename_workspace("main").unwrap();
        core.projects[0].project.name = "home".into();
        // Tests count a window's columns; those of the gutter turn it on.
        core.settings.line_numbers = crate::settings::LineNumbers::Off;
        let mut harness = Harness {
            core,
            dir,
            now: Instant::now(),
            clipboard: None,
            quit: false,
            recovery: HashMap::new(),
            deleted: Vec::new(),
            greps: Vec::new(),
            reads: HashMap::new(),
            listings: HashMap::new(),
            indexes: HashMap::new(),
            directories: HashMap::new(),
            checks: HashMap::new(),
            heads: HashMap::new(),
            saves: std::collections::VecDeque::new(),
            writes: Vec::new(),
            reverts: HashMap::new(),
            diffs: HashMap::new(),
            hits: std::collections::VecDeque::new(),
            watched: WatchSet::default(),
            collect_effects: false,
            effects: Vec::new(),
            terminal_ops: Vec::new(),
            agent_ops: Vec::new(),
        };
        for (name, text) in files {
            harness.read_reply(name, Some(text.as_bytes()));
        }
        harness
    }

    pub fn read_reply(&mut self, name: &str, bytes: Option<&[u8]>) {
        let path = self.path(name);
        self.checks.insert(path.clone(), bytes.map(hash));
        self.reads
            .insert(path, Ok(bytes.map(|b| (b.to_vec(), hash(b)))));
    }

    pub fn listing(&mut self, dir: &str, names: &[&str]) {
        self.listings.insert(
            self.path(dir),
            Ok(names
                .iter()
                .map(|s| crate::FileEntry {
                    name: s.to_string(),
                    annotation: String::new(),
                })
                .collect()),
        );
    }

    pub fn index_reply(&mut self, root: &str, files: &[&str], directories: &[&str]) {
        self.indexes.insert(
            self.path(root),
            Ok(files.iter().map(PathBuf::from).collect()),
        );
        self.directories.insert(
            self.path(root),
            directories.iter().map(|d| self.path(d)).collect(),
        );
    }

    pub fn save_reply(&mut self, text: &str) {
        self.saves.push_back(Ok(hash(text.as_bytes())));
    }

    pub fn grep_reply(&mut self, hits: &[(&str, &str, usize, usize, &str)]) {
        self.hits.push_back(Ok(hits
            .iter()
            .map(|(root, path, line, col, text)| Hit {
                root: self.path(root),
                path: (*path).into(),
                line: *line,
                col: *col,
                text: (*text).into(),
            })
            .collect()));
    }

    pub fn new_workspace(&mut self, name: &str, project: &str) -> crate::workspace::WorkspaceId {
        let project = self.core.project_id(project).unwrap_or_else(|_| {
            self.core
                .add_project(crate::project::Loaded::ad_hoc(project, self.dir.as_path()))
        });
        self.core.new_workspace(name, project)
    }

    pub fn path(&self, name: &str) -> PathBuf {
        self.dir.as_path().join(name)
    }

    /// Feeds a `kbd`-style script such as `"hello C-x C-s"`, delivering configured replies.
    /// Plain chars arrive as typed text, the way the platform's text input sends them.
    pub fn keys(&mut self, script: &str) -> &mut Self {
        for key in parse_keys(script).unwrap() {
            let msg = match key.code {
                KeyCode::Char(c) if key == Key::plain(key.code) => Msg::Insert {
                    text: c.into(),
                    replace: None,
                },
                _ => {
                    // As the app does, the clipboard goes in only before a key that wants it.
                    if let Some(text) = self.clipboard.clone()
                        && self.core.wants_clipboard(key)
                    {
                        self.send(Msg::Clipboard(text));
                    }
                    Msg::Key(key)
                }
            };
            self.send(msg);
        }
        self
    }

    /// Sends `msg`, delivering configured replies and collecting unanswered effects.
    pub fn send(&mut self, msg: Msg) -> &mut Self {
        let effects = self.core.handle(self.now, msg);
        self.run(effects);
        self.settle();
        self
    }

    /// Waits for the completion matcher's threads, so tests see settled matches.
    fn settle(&mut self) {
        let mut effects = Vec::new();
        self.core.poll_completion(self.now, &mut effects);
        self.run(effects);
        let core = &mut self.core;
        while let (Some(_), Some(matcher)) = (&core.prompts.minibuffer, &mut core.prompts.matcher)
            && matcher.tick(50).running
        {}
        let mut effects = Vec::new();
        self.core.poll_completion(self.now, &mut effects);
        self.core.preview(&mut effects);
        self.run(effects);
    }

    /// Calls `name` with `args`, delivering configured replies.
    pub fn call(&mut self, name: &str, args: Args) -> &mut Self {
        let mut ctx = self.ctx();
        ctx.call(name, args);
        let effects = ctx.effects;
        self.run(effects);
        self.settle();
        self
    }

    pub(crate) fn run(&mut self, effects: Effects) {
        if self.collect_effects {
            self.effects.extend(effects);
            return;
        }
        // File replies arrive after the entire effect batch in the app.
        for effect in &effects {
            if let Effect::Watch(set) = effect {
                self.watched = set.clone();
            }
        }
        for effect in effects {
            match effect {
                Effect::Terminal(op) => self.terminal_ops.push(op),
                Effect::Agent(op) => self.agent_ops.push(op),
                Effect::File(op) => {
                    let reply = match &op {
                        FileOp::Save {
                            buffer,
                            tick,
                            path,
                            text,
                            ..
                        } => {
                            self.writes.push((path.clone(), text.to_string()));
                            self.saves.pop_front().map(|result| Msg::Saved {
                                buffer: *buffer,
                                tick: *tick,
                                result,
                            })
                        }
                        FileOp::ListDir { dir, request } => {
                            self.listings.get(dir).cloned().map(|result| Msg::Listed {
                                request: *request,
                                result,
                            })
                        }
                        FileOp::WriteProject {
                            intent, path, text, ..
                        } => Some(Msg::ProjectWritten {
                            intent: *intent,
                            result: Ok(crate::project::Loaded {
                                project: crate::project::Project::parse(text).unwrap(),
                                file: Some((path.clone(), text.clone())),
                            }),
                        }),
                        FileOp::Index {
                            project,
                            walk,
                            roots,
                            ..
                        } => roots
                            .iter()
                            .map(|root| {
                                self.indexes
                                    .get(root)
                                    .cloned()
                                    .map(|result| (root.clone(), result))
                            })
                            .collect::<Option<Vec<_>>>()
                            .map(|roots| Msg::Indexed {
                                project: *project,
                                walk: *walk,
                                directories: roots
                                    .iter()
                                    .flat_map(|(root, _)| {
                                        self.directories.get(root).into_iter().flatten().cloned()
                                    })
                                    .collect(),
                                roots,
                            }),
                        FileOp::Revert { buffer, tick, .. } => self
                            .reverts
                            .get(buffer)
                            .cloned()
                            .map(|result| Msg::Reverted {
                                buffer: *buffer,
                                tick: *tick,
                                result,
                            }),
                        FileOp::Diff { buffer, path, .. } => {
                            self.diffs.get(path).cloned().map(|result| Msg::Diffed {
                                buffer: *buffer,
                                result,
                            })
                        }
                        FileOp::Open { intent, path } => {
                            self.reads.get(path).cloned().map(|result| Msg::Opened {
                                intent: *intent,
                                path: path.clone(),
                                result,
                            })
                        }
                        FileOp::Preview(request) => {
                            self.reads
                                .get(&request.path)
                                .cloned()
                                .map(|read| Msg::Previewed {
                                    request: request.clone(),
                                    result: read.and_then(|file| {
                                        file.map(|(bytes, _)| {
                                            crate::buffer::prepare_preview(&bytes)
                                                .map_err(str::to_string)
                                        })
                                        .transpose()
                                    }),
                                })
                        }
                        FileOp::Annotate { .. } => None,
                        FileOp::Head(paths) => Some(Msg::Heads(
                            (paths.iter())
                                .map(|path| (path.clone(), self.heads.get(path).cloned()))
                                .collect(),
                        )),
                        FileOp::LanguageVersion {
                            buffer,
                            request,
                            path,
                            language,
                        } => {
                            let version = crate::language_version::candidates(path, language)
                                .into_iter()
                                .find_map(|candidate| {
                                    let (bytes, _) =
                                        self.reads.get(&candidate)?.as_ref().ok()?.as_ref()?;
                                    let text = std::str::from_utf8(bytes).ok()?;
                                    crate::language_version::parse(&candidate, language, text)
                                })
                                .filter(|s| !s.is_empty());
                            Some(Msg::LanguageVersion {
                                buffer: *buffer,
                                request: *request,
                                version,
                            })
                        }
                        FileOp::Check(paths) => paths
                            .iter()
                            .map(|path| self.checks.get(path).map(|hash| (path.clone(), *hash)))
                            .collect::<Option<Vec<_>>>()
                            .map(Msg::Checked),
                    };
                    match reply {
                        Some(reply) => {
                            self.send(reply);
                        }
                        None => self.effects.push(Effect::File(op)),
                    }
                }
                Effect::Watch(_) => {}
                effect @ Effect::CaptureEnvironment { .. } => self.effects.push(effect),
                Effect::SetClipboard(text) => self.clipboard = Some(text),
                Effect::Quit => self.quit = true,
                Effect::Recovery(RecoveryOp::AutoSave {
                    buffer,
                    name,
                    path,
                    text,
                    replaces,
                    ..
                }) => {
                    (self.recovery).insert(buffer, (name, path, text.to_string()));
                    self.deleted.extend(replaces);
                }
                Effect::Recovery(RecoveryOp::DeleteAutoSave(buffer)) => {
                    assert!(!self.quit, "deleted after the quit");
                    self.recovery
                        .remove(&buffer)
                        .expect("deletes a written file");
                }
                Effect::Recovery(RecoveryOp::DeleteRecovery(file)) => self.deleted.push(file),
                Effect::Grep(GrepOp::Search(op)) => {
                    self.greps.push(Some(op.pattern.clone()));
                    if let Some(hits) = self.hits.pop_front() {
                        self.send(Msg::Grepped {
                            search: op.id,
                            hits,
                            done: true,
                        });
                    }
                }
                Effect::Grep(GrepOp::Cancel) => self.greps.push(None),
                Effect::LoadConfiguration { request } => {
                    let settings = self.fixture_text("settings.toml");
                    let keymap = self.fixture_text("keymap.toml");
                    self.send(Msg::ConfigurationLoaded {
                        request,
                        bundle: crate::configuration::Bundle {
                            settings: Ok(settings),
                            keymap: Ok(keymap),
                            projects_dir: self.core.projects_dir.clone(),
                            projects: Vec::new(),
                        },
                    });
                }
            }
        }
    }

    /// Visits prepared fixture bytes without filesystem work.
    pub fn open(&mut self, name: &str) -> &mut Self {
        let path = self.path(name);
        let file = self.reads.get(&path).cloned().unwrap_or(Ok(None)).unwrap();
        self.core.open(&path, file);
        self
    }

    pub fn submitted(&self, name: &str) -> String {
        self.writes
            .iter()
            .rev()
            .find(|(path, _)| *path == self.path(name))
            .expect("a save was requested")
            .1
            .clone()
    }

    pub fn prepare_revert(&mut self, name: &str, bytes: Option<&[u8]>) {
        let path = self.path(name);
        let read = Ok(bytes.map(|b| (b.to_vec(), hash(b))));
        for (id, buffer) in self
            .core
            .buffers
            .recent()
            .filter(|(_, b)| b.file.as_ref() == Some(&path))
        {
            self.reverts.insert(
                id,
                crate::buffer::revert(&path, buffer.text(), read.clone()),
            );
        }
    }

    pub fn fixture_text(&self, name: &str) -> Option<String> {
        self.reads
            .get(&self.path(name))
            .and_then(|r| r.as_ref().ok())
            .and_then(|r| r.as_ref())
            .map(|(bytes, _)| String::from_utf8(bytes.clone()).unwrap())
    }
}

#[test]
fn collected_saves_can_be_answered_in_reverse_order_across_workspaces() {
    let mut h = Harness::new(&[]);
    h.collect_effects = true;
    let first = h.core.workspace();
    let path = h.path("held.txt");
    h.core
        .open(&path, Some((b"original".to_vec(), hash(b"original"))));
    h.keys("first SPC C-x C-s");
    let first_buffer = h.core.windows.window.buffer();
    let second = h.new_workspace("second", "home");
    h.core
        .open(&path, Some((b"original".to_vec(), hash(b"original"))));
    h.keys("second SPC C-x C-s");
    let second_buffer = h.core.windows.window.buffer();
    let replies: Vec<_> = std::mem::take(&mut h.effects)
        .into_iter()
        .filter_map(|effect| match effect {
            Effect::File(FileOp::Save {
                buffer, tick, text, ..
            }) => Some(Msg::Saved {
                buffer,
                tick,
                result: Ok(hash(text.to_string().as_bytes())),
            }),
            _ => None,
        })
        .collect();
    assert_eq!(replies.len(), 2);
    for reply in replies.into_iter().rev() {
        h.send(reply);
    }
    assert_eq!(h.core.workspace(), second);
    assert_eq!(h.text(), "second original\n");
    assert_eq!(
        h.core.buffers.get(second_buffer).unwrap().disk,
        Some(hash(b"second original\n"))
    );
    h.core.switch_workspace(first).unwrap();
    assert_eq!(h.text(), "first original\n");
    assert_eq!(
        h.core.buffers.get(first_buffer).unwrap().disk,
        Some(hash(b"first original\n"))
    );
}

impl Harness {
    /// Moves the fake clock on and wakes the core, as the app does at a deadline.
    pub fn advance(&mut self, d: Duration) -> &mut Self {
        self.now += d;
        let effects = self.core.handle(self.now, Msg::Tick);
        self.run(effects);
        self
    }

    pub(crate) fn ctx(&mut self) -> Ctx<'_> {
        Ctx {
            core: &mut self.core,
            now: self.now,
            effects: Vec::new(),
        }
    }

    pub fn start_timer(&mut self, after: Duration, repeat: Option<Duration>, msg: Msg) -> TimerId {
        self.ctx().start_timer(after, repeat, msg)
    }

    pub fn cancel_timer(&mut self, id: TimerId) {
        self.ctx().cancel_timer(id)
    }

    pub fn text(&self) -> String {
        self.core.text()
    }

    pub fn point(&self) -> usize {
        self.core.point()
    }

    /// The display model for a window `rows` text lines high.
    pub fn display(&mut self, rows: usize) -> WindowDisplay {
        self.core.display(rows)
    }

    /// The frame display after the frontend reports a windows area `width` by `height` px
    /// with 8 px cells.
    pub fn frame(&mut self, width: f32, height: f32) -> FrameDisplay {
        let cell_width = 8.;
        self.send(Msg::Resize {
            width,
            height,
            cell_width,
        });
        self.core.frame()
    }
}

pub(crate) const MS: Duration = Duration::from_millis(1);

pub(crate) fn msg(c: char) -> Msg {
    Msg::Key(Key::plain(KeyCode::Char(c)))
}

/// Registers `test-cmd` with `spec`; it shows the args it got on the message line.
pub(crate) fn test_cmd(h: &mut Harness, spec: ArgSpec) {
    h.core.register(Command {
        name: "test-cmd",
        spec,
        handler: |ctx, args| {
            ctx.core.message = Some(format!("{args:?}"));
            Ok(())
        },
    });
}

/// A harness visiting `text`, with point at 0.
pub(crate) fn on(text: &str) -> Harness {
    let mut h = Harness::new(&[("a.txt", text)]);
    h.open("a.txt");
    h
}

pub(crate) fn line_start(h: &Harness, line: usize) -> usize {
    h.core.current().char_idx(line, 0).unwrap()
}

/// Twenty lines `0` to `19`, shown 5 at a time.
pub(crate) fn twenty_lines() -> Harness {
    let text: Vec<String> = (0..20).map(|i| i.to_string()).collect();
    let mut h = on(&text.join("\n"));
    h.display(5);
    h
}

/// The mark and whether it is active.
pub(crate) fn mark(h: &Harness) -> (Option<usize>, bool) {
    (
        h.core.windows.window.mark(&h.core.buffers),
        h.core.windows.window.active(),
    )
}

pub(crate) fn bind_mode(h: &mut Harness, mode: &'static str, keys: &str, command: &'static str) {
    let map = h.core.keymaps.entry(mode.into()).or_default();
    map.bind(keys, command).unwrap();
}

pub(crate) fn buffer(h: &mut Harness) -> &mut crate::buffer::Buffer {
    let id = h.core.windows.window.buffer();
    h.core.buffers.get_mut(id).unwrap()
}

/// Binds `C-c t` to `test-insert`, which inserts `hi` and shows this and last command.
pub(crate) fn insert_cmd(h: &mut Harness) {
    h.core.register(Command {
        name: "test-insert",
        spec: ArgSpec::default(),
        handler: |ctx, _| {
            let core = &mut *ctx.core;
            core.message = Some(format!(
                "{:?} {:?}",
                core.this_command(),
                core.last_command()
            ));
            let point = core.windows.window.point(&core.buffers);
            let id = core.windows.window.buffer();
            core.buffers
                .get_mut(id)
                .unwrap()
                .insert(point, "hi")
                .unwrap();
            core.windows
                .window
                .set_point(&mut core.buffers, point + 2)
                .unwrap();
            Ok(())
        },
    });
    bind_mode(h, "global", "C-c t", "test-insert");
}

/// Registers `ask`, prompting for `prompts` and showing its args, bound to `C-c p`.
pub(crate) fn ask(h: &mut Harness, prompts: &'static [&'static str]) {
    test_cmd(
        h,
        ArgSpec {
            prompts,
            ..ArgSpec::default()
        },
    );
    bind_mode(h, "global", "C-c p", "test-cmd");
}

pub(crate) fn strings(strings: &[&str]) -> String {
    let strings: Vec<String> = strings.iter().map(|s| s.to_string()).collect();
    format!("{strings:?}")
}

/// Registers `ask` completing over `candidates` at the prompt `Pick: `, bound to `C-c p`.
pub(crate) fn ask_completing(h: &mut Harness, candidates: fn(&Core, &Args) -> Vec<String>) {
    test_cmd(
        h,
        ArgSpec {
            prompts: &["Pick: "],
            completion: Some(candidates),
            ..ArgSpec::default()
        },
    );
    bind_mode(h, "global", "C-c p", "test-cmd");
}

pub(crate) fn fruit(_: &Core, _: &Args) -> Vec<String> {
    ["apple", "banana", "cherry"].map(String::from).into()
}

pub(crate) fn listed(h: &mut Harness) -> (Vec<String>, Option<usize>) {
    let mb = h.core.minibuffer().unwrap();
    (mb.candidates, mb.selected)
}

pub(crate) fn name(h: &Harness) -> &str {
    h.core.current().name.as_str()
}
