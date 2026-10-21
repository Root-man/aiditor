//! Headless core driven by key scripts and a fake clock, for tests.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use tempfile::TempDir;

use crate::{
    Core, Ctx, Effect, Effects, FileOp, Msg, RecoveryOp,
    buffer::{BufferId, encode, hash},
    command::{ArgSpec, Args, Command},
    display::{FrameDisplay, WindowDisplay},
    key::{Key, KeyCode, parse_keys},
    timer::TimerId,
};

pub struct Harness {
    pub core: Core,
    pub dir: TempDir,
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
}

impl Harness {
    /// A core plus a temp dir holding `files` as (relative path, contents).
    pub fn new(files: &[(&str, &str)]) -> Self {
        let dir = tempfile::tempdir().unwrap();
        for (name, contents) in files {
            let path = dir.path().join(name);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, contents).unwrap();
        }
        let mut core = Core::default();
        // Named the same whatever `$HOME` is, so scratch names, titles and snapshots don't vary.
        core.rename_workspace("main").unwrap();
        core.projects[0].project.name = "home".into();
        core.workspaces[0].project = "home".into();
        Harness {
            core,
            dir,
            now: Instant::now(),
            clipboard: None,
            quit: false,
            recovery: HashMap::new(),
            deleted: Vec::new(),
        }
    }

    pub fn path(&self, name: &str) -> PathBuf {
        self.dir.path().join(name)
    }

    /// Feeds a `kbd`-style script such as `"hello C-x C-s"`, carrying out effects as the app does.
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

    /// Sends `msg` to the core, carrying out its effects as the app does.
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
    }

    /// Calls the command `name` with `args`, carrying out its effects as the app does.
    pub fn call(&mut self, name: &str, args: Args) -> &mut Self {
        let mut ctx = self.ctx();
        ctx.call(name, args);
        let effects = ctx.effects;
        self.run(effects);
        self.settle();
        self
    }

    pub(crate) fn run(&mut self, effects: Effects) {
        for effect in effects {
            match effect {
                // As the file service does, minus its atomic write.
                Effect::File(FileOp::Save {
                    buffer,
                    tick,
                    path,
                    text,
                    crlf,
                    bom,
                    hash: expected,
                }) => {
                    let bytes = encode(&text, crlf, bom);
                    let on_disk = std::fs::read(&path).ok().map(|b| hash(&b));
                    let result = match on_disk == expected {
                        false => Err(format!("{} changed on disk; not saved", path.display())),
                        true => std::fs::write(&path, &bytes)
                            .map(|()| hash(&bytes))
                            .map_err(|err| format!("{}: {err}", path.display())),
                    };
                    let msg = Msg::Saved {
                        buffer,
                        tick,
                        result,
                    };
                    self.send(msg);
                }
                // As `files::list` does; the core can't depend on the services crate.
                Effect::File(FileOp::ListDir(dir)) => {
                    let result = std::fs::read_dir(&dir)
                        .map(|entries| {
                            let mut names: Vec<String> = (entries.flatten())
                                .map(|e| {
                                    let slash = if e.path().is_dir() { "/" } else { "" };
                                    format!("{}{slash}", e.file_name().to_string_lossy())
                                })
                                .collect();
                            names.sort();
                            names
                        })
                        .map_err(|err| format!("{}: {err}", dir.display()));
                    self.send(Msg::Listed { dir, result });
                }
                // As `projects::write` does, minus its check of the roots.
                Effect::File(FileOp::WriteProject { path, old, text }) => {
                    if std::fs::read_to_string(&path).ok() != old {
                        let err =
                            format!("{} changed on disk; reload with C-x C-r", path.display());
                        self.send(Msg::ProjectWritten(Err(err)));
                        continue;
                    }
                    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
                    std::fs::write(&path, &text).unwrap();
                    let project = crate::project::Project::parse(&text).unwrap();
                    let file = Some((path, text));
                    self.send(Msg::ProjectWritten(Ok(crate::project::Loaded {
                        project,
                        file,
                    })));
                }
                // As `files::index` does, minus the ignore files.
                Effect::File(FileOp::Index {
                    project,
                    walk,
                    roots,
                }) => {
                    let roots = (roots.into_iter())
                        .map(|root| {
                            let mut files = Vec::new();
                            let result = walk_files(&root, &root, &mut files)
                                .map(|()| {
                                    files.sort();
                                    files
                                })
                                .map_err(|err| format!("{}: {err}", root.display()));
                            (root, result)
                        })
                        .collect();
                    self.send(Msg::Indexed {
                        project,
                        walk,
                        roots,
                    });
                }
                Effect::File(FileOp::Open(path)) => {
                    let result = match std::fs::read(&path) {
                        Ok(bytes) => {
                            let h = hash(&bytes);
                            Ok(Some((bytes, h)))
                        }
                        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
                        Err(err) => Err(format!("{}: {err}", path.display())),
                    };
                    self.send(Msg::Opened { path, result });
                }
                Effect::SetClipboard(text) => self.clipboard = Some(text),
                Effect::Quit => self.quit = true,
                Effect::Recovery(RecoveryOp::AutoSave {
                    buffer,
                    name,
                    path,
                    text,
                    ..
                }) => _ = (self.recovery).insert(buffer, (name, path, text.to_string())),
                Effect::Recovery(RecoveryOp::DeleteAutoSave(buffer)) => {
                    assert!(!self.quit, "deleted after the quit");
                    self.recovery
                        .remove(&buffer)
                        .expect("deletes a written file");
                }
                Effect::Recovery(RecoveryOp::DeleteRecovery(file)) => self.deleted.push(file),
                Effect::LoadSettings => {
                    let text = std::fs::read_to_string(self.path("settings.toml")).ok();
                    self.core.handle(self.now, Msg::SettingsLoaded(Ok(text)));
                    let text = std::fs::read_to_string(self.path("keymap.toml")).ok();
                    self.core.handle(self.now, Msg::KeymapLoaded(Ok(text)));
                }
            }
        }
    }

    /// Visits `name` in the temp dir, reading it as the app does; a missing file is empty.
    pub fn open(&mut self, name: &str) -> &mut Self {
        let path = self.path(name);
        let file = std::fs::read(&path).ok().map(|b| (hash(&b), b));
        self.core.open(&path, file.map(|(h, b)| (b, h)));
        self
    }

    pub fn disk(&self, name: &str) -> String {
        std::fs::read_to_string(self.path(name)).unwrap()
    }
}

/// Adds the files under `dir`, relative to `root`, skipping `.git`.
fn walk_files(root: &Path, dir: &Path, files: &mut Vec<PathBuf>) -> std::io::Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if !entry.file_type()?.is_dir() {
            files.push(path.strip_prefix(root).unwrap().to_path_buf());
        } else if !path.ends_with(".git") {
            walk_files(root, &path, files)?;
        }
    }
    Ok(())
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

    /// The display model for a window `rows` text lines high, plus its mode line.
    pub fn display(&mut self, rows: usize) -> WindowDisplay {
        let height = (rows + 1) as f32 * self.core.settings().line_height();
        self.core.display(height)
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
    (h.core.window.mark(&h.core.buffers), h.core.window.active())
}

pub(crate) fn bind_mode(h: &mut Harness, mode: &'static str, keys: &str, command: &'static str) {
    let map = h.core.keymaps.entry(mode.into()).or_default();
    map.bind(keys, command).unwrap();
}

pub(crate) fn buffer(h: &mut Harness) -> &mut crate::buffer::Buffer {
    let id = h.core.window.buffer();
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
            let point = core.window.point(&core.buffers);
            let id = core.window.buffer();
            core.buffers
                .get_mut(id)
                .unwrap()
                .insert(point, "hi")
                .unwrap();
            core.window.set_point(&mut core.buffers, point + 2).unwrap();
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
