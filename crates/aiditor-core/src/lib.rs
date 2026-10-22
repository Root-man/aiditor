//! Editor state. A plain state machine with no GPUI or Tokio dependency.

// Commands are contained with `catch_unwind`, which aborting would silently disable.
#[cfg(panic = "abort")]
compile_error!("aiditor needs panic = \"unwind\": commands are contained with catch_unwind");

mod autosave;
mod bindings;
pub mod buffer;
mod builtins;
pub mod command;
pub mod display;
mod editing;
mod frame;
mod grep;
#[cfg(test)]
mod harness;
mod jump;
pub mod key;
pub mod keymap;
mod kill_ring;
mod mark;
mod minibuffer;
mod motion;
mod prefix;
pub mod project;
mod project_commands;
pub mod settings;
mod swiper;
pub mod timer;
mod visiting;
mod watching;
pub mod window;
pub mod workspace;

use std::backtrace::Backtrace;
use std::cell::RefCell;
use std::collections::HashMap;
use std::ops::Range;
use std::panic::{self, AssertUnwindSafe};
use std::path::PathBuf;
use std::time::{Duration, Instant};

use ropey::Rope;

use bindings::HINT_DELAY;
use buffer::{Buffer, BufferId, Buffers};
use command::{Args, Command, Prefix};
use key::{Key, KeyCode};
use keymap::{Binding, Keymap};
use kill_ring::KillRing;
use minibuffer::Prompts;
use project::{Loaded, Project};
use project_commands::FileIndex;
use settings::Settings;
use timer::{TimerId, Timers};
use window::{Tree, Window, WindowId};
use workspace::{Workspace, WorkspaceId};

#[derive(Clone, Debug, PartialEq)]
pub enum Msg {
    Key(Key),
    /// Text from the platform's text input, each char typed as a plain key. `replace`, in UTF-16
    /// code units, is text an input method rewrites, deleted first.
    Insert {
        text: String,
        replace: Option<Range<usize>>,
    },
    /// The wheel scrolled `window` this many lines down its buffer, up when negative.
    Scroll {
        window: WindowId,
        lines: i64,
    },
    /// A left click in `window` on the char at this buffer line and column.
    Click {
        window: WindowId,
        line: usize,
        col: usize,
    },
    /// The frame's windows area is `width` by `height` px and the font's cells are
    /// `cell_width` px wide.
    Resize {
        width: f32,
        height: f32,
        cell_width: f32,
    },
    /// Wakes the core when a timer is due; due timers fire on every message.
    Tick,
    /// The file service's answer to `FileOp::Save`: the file's new hash, or why it wasn't written.
    Saved {
        buffer: BufferId,
        tick: u64,
        result: Result<u64, String>,
    },
    /// The file service's answer to `FileOp::Revert`: what turns the buffer's text at `tick` into
    /// the file's, or why it can't.
    Reverted {
        buffer: BufferId,
        tick: u64,
        result: Result<buffer::Revert, String>,
    },
    /// The file service's answer to `FileOp::Diff`: the unified diff, empty for none, or why
    /// there is none.
    Diffed {
        buffer: BufferId,
        result: Result<String, String>,
    },
    /// The file service's answer to `FileOp::ListDir`: the directory's entries, or why it
    /// couldn't be listed.
    Listed {
        dir: PathBuf,
        result: Result<Vec<String>, String>,
    },
    /// The file service's answer to `FileOp::Open`: the file's bytes and hash, `None` when there
    /// is no file, or why it couldn't be read.
    Opened {
        path: PathBuf,
        result: Result<Option<(Vec<u8>, u64)>, String>,
    },
    /// The file service's answer to `FileOp::Preview`, as to `FileOp::Open`.
    Previewed {
        path: PathBuf,
        result: Result<Option<(Vec<u8>, u64)>, String>,
    },
    /// The file service's answer to `FileOp::Index`: each root with its files, relative and
    /// sorted, or why it couldn't be walked.
    Indexed {
        project: String,
        walk: u64,
        roots: Vec<(PathBuf, Result<Vec<PathBuf>, String>)>,
    },
    /// The watcher's debounced report: the paths that may have changed and what each is now, and
    /// whether events were lost, so every visited file needs checking.
    FilesChanged {
        paths: Vec<(PathBuf, Disk)>,
        rescan: bool,
    },
    /// The file service's answer to `FileOp::Check`: each readable file's hash, `None` when gone.
    Checked(Vec<(PathBuf, Option<u64>)>),
    /// The grep service's hits for search `search`, in batches as they are found, or why the
    /// pattern is invalid; `done` on its last message. A cancelled search sends nothing more.
    Grepped {
        search: u64,
        hits: Result<Vec<Hit>, String>,
        done: bool,
    },
    /// The user's settings file: its text, `None` if it doesn't exist, or why it couldn't be read.
    SettingsLoaded(Result<Option<String>, String>),
    /// The user's keymap file, the same way.
    KeymapLoaded(Result<Option<String>, String>),
    /// Every project file in the projects directory `dir`, parsed and checked, or why it was
    /// rejected; `dir` is `None` without a config dir.
    ProjectsLoaded {
        dir: Option<PathBuf>,
        projects: Vec<Result<Loaded, String>>,
    },
    /// The file service's answer to `FileOp::WriteProject`: the project as written, or why it
    /// wasn't, the error starting with the file's name.
    ProjectWritten(Result<Loaded, String>),
    /// The prefix hint's delay passed with the prefix still pending.
    Hint,
    /// The system clipboard's text, sent before a key `Core::wants_clipboard` accepts.
    Clipboard(String),
    /// A line the app logged, for `*Messages*`.
    Log(String),
    /// The window was asked to close; quits as `C-x C-c` does.
    Quit,
    /// An auto-save timer fired: write recovery files for buffers edited since the last one.
    AutoSave,
    /// Recovery files an earlier session left, found at launch: offer each one.
    Recover(Vec<Recovered>),
}

/// A recovery file read back: what `RecoveryOp::AutoSave` wrote, plus the visited file as it is now.
#[derive(Clone, Debug, PartialEq)]
pub struct Recovered {
    /// The recovery file itself.
    pub file: PathBuf,
    pub name: String,
    pub path: Option<PathBuf>,
    /// The hash of the disk version the text was based on.
    pub hash: Option<u64>,
    pub text: String,
    /// The visited file's bytes and hash now, `None` when there is no file.
    pub disk: Option<(Vec<u8>, u64)>,
}

/// What a path the watcher reports is on disk now.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Disk {
    Gone,
    File,
    Dir,
}

/// What the watcher should watch: project roots, each with its project's ignore patterns, and
/// the files buffers visit.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct WatchSet {
    pub roots: Vec<(PathBuf, Vec<String>)>,
    pub files: Vec<PathBuf>,
}

/// A line matching a grep search.
#[derive(Clone, Debug, PartialEq)]
pub struct Hit {
    pub root: PathBuf,
    /// The file, relative to `root`.
    pub path: PathBuf,
    /// 0-based.
    pub line: usize,
    /// The char column where the line's first match starts, 0-based.
    pub col: usize,
    /// The line without its line ending.
    pub text: String,
}

/// Chains a panic hook, once, that keeps each panic's report and backtrace for `Ctx::try_call`.
static PANIC_HOOK: std::sync::Once = std::sync::Once::new();

thread_local! {
    static PANIC: RefCell<Option<String>> = const { RefCell::new(None) };
}

/// Work the app carries out for the core.
pub enum Effect {
    /// A visited file's read, write or listing, answered with a message.
    File(FileOp),
    /// A project grep, answered with `Msg::Grepped` batches.
    Grep(GrepOp),
    /// Watch this set from now on, answered with `Msg::FilesChanged` reports.
    Watch(WatchSet),
    /// A recovery file's write or delete, carried out off the UI thread with no answer.
    Recovery(RecoveryOp),
    /// Read the user's settings, keymap and project files and send `Msg::SettingsLoaded`,
    /// `Msg::KeymapLoaded` and `Msg::ProjectsLoaded`.
    LoadSettings,
    /// Put this text on the system clipboard.
    SetClipboard(String),
    /// Quit the app.
    Quit,
}

/// File work for the file service; each sends one reply message.
pub enum FileOp {
    /// Write `text`, encoded with `crlf` and `bom`, to `path` if the file still hashes to `hash`
    /// (`None`: no file), then send `Msg::Saved` with `buffer` and its `tick` at the save.
    Save {
        buffer: BufferId,
        tick: u64,
        path: PathBuf,
        text: Rope,
        crlf: bool,
        bom: bool,
        hash: Option<u64>,
    },
    /// Read the file at `path`, diff `text`, `buffer`'s text at `tick`, against it with
    /// `buffer::revert` and send `Msg::Reverted`.
    Revert {
        buffer: BufferId,
        tick: u64,
        path: PathBuf,
        text: Rope,
    },
    /// Read the file at `path`, diff it against `text`, the text of `buffer` named `name`, with
    /// `buffer::unified_diff` and send `Msg::Diffed`.
    Diff {
        buffer: BufferId,
        name: String,
        path: PathBuf,
        text: Rope,
    },
    /// List the directory and send `Msg::Listed`.
    ListDir(PathBuf),
    /// Read the file and send `Msg::Opened`.
    Open(PathBuf),
    /// Read the file and send `Msg::Previewed`.
    Preview(PathBuf),
    /// Hash each file and send `Msg::Checked`.
    Check(Vec<PathBuf>),
    /// Walk every root of `project`, leaving out what its `ignore` patterns match, and send
    /// `Msg::Indexed` with `walk`.
    Index {
        project: String,
        walk: u64,
        roots: Vec<PathBuf>,
        ignore: Vec<String>,
    },
    /// Write the project file `text` to `path` if it is a valid project and the file is still
    /// `old` (`None`: no file, making the directory), then send `Msg::ProjectWritten`.
    WriteProject {
        path: PathBuf,
        old: Option<String>,
        text: String,
    },
}

/// Work for the grep service.
pub enum GrepOp {
    /// Cancel the running search and start this one.
    Search(GrepSearch),
    /// Cancel the running search.
    Cancel,
}

/// Search the files the index lists under `roots`, with the project's `ignore` patterns, for the
/// regexp `pattern`, smart case, until `limit` hits; numbered `id`, from 1.
pub struct GrepSearch {
    pub id: u64,
    pub roots: Vec<PathBuf>,
    pub ignore: Vec<String>,
    pub pattern: String,
    pub limit: usize,
}

/// Recovery file work for the recovery thread.
pub enum RecoveryOp {
    /// Write `buffer`'s recovery file: its `name`, visited `path`, base disk `hash` and `text`.
    AutoSave {
        buffer: BufferId,
        name: String,
        path: Option<PathBuf>,
        hash: Option<u64>,
        text: Rope,
    },
    /// Delete `buffer`'s recovery file.
    DeleteAutoSave(BufferId),
    /// Delete an earlier session's recovery file, recovered or discarded.
    DeleteRecovery(PathBuf),
}

pub type Effects = Vec<Effect>;

/// What a command gets: the core, the time the message is handled and the effects so far.
pub struct Ctx<'a> {
    pub core: &'a mut Core,
    pub now: Instant,
    pub effects: Effects,
}

impl Ctx<'_> {
    pub fn start_timer(&mut self, after: Duration, repeat: Option<Duration>, msg: Msg) -> TimerId {
        self.core.timers.start(self.now, after, repeat, msg)
    }

    pub fn cancel_timer(&mut self, id: TimerId) {
        self.core.timers.cancel(id)
    }

    /// Runs the command `name` with `args`, filling the region from point and mark if the
    /// command needs it; any error goes to the message line.
    pub fn call(&mut self, name: &str, args: Args) {
        let (buffer, tick) = (self.core.window.buffer(), self.core.current().tick());
        if let Err(err) = self.try_call(name, args) {
            self.core.echo(err);
        }
        // A command that changes the buffer deactivates the mark, as in Emacs.
        if self.core.window.buffer() == buffer && self.core.current().tick() != tick {
            self.core.window.set_active(false);
        }
    }

    fn try_call(&mut self, name: &str, mut args: Args) -> Result<(), String> {
        let command = *self
            .core
            .commands
            .get(name)
            .ok_or_else(|| format!("No command named {name}"))?;
        if command.spec.region && args.region.is_none() {
            let core = &*self.core;
            let point = core.window.point(&core.buffers);
            let mark = core
                .window
                .mark(&core.buffers)
                .ok_or("The mark is not set now, so there is no region")?;
            args.region = Some(point.min(mark)..point.max(mark));
        }
        if let Some(&prompt) = command.spec.prompts.get(args.strings.len()) {
            return self.read_string(command, args, prompt.into(), None);
        }
        PANIC_HOOK.call_once(|| {
            let previous = panic::take_hook();
            panic::set_hook(Box::new(move |info| {
                let report = format!("{info}\n{}", Backtrace::force_capture());
                PANIC.with(|p| *p.borrow_mut() = Some(report));
                previous(info);
            }));
        });
        let result = panic::catch_unwind(AssertUnwindSafe(|| (command.handler)(self, args)));
        result.unwrap_or_else(|_| {
            let report = (PANIC.with(|p| p.borrow_mut().take()))
                .unwrap_or_else(|| "No report: another panic hook replaced the core's".into());
            let core = &mut *self.core;
            core.echo(format!(
                "{name} panicked; its state may be inconsistent (see *Messages*)"
            ));
            core.log(&report);
            Ok(())
        })
    }

    fn dispatch(&mut self, msg: Msg) {
        match msg {
            Msg::Key(key) => {
                self.key(key);
                // The clipboard text was for this key only.
                self.core.kills.clipboard = None;
            }
            Msg::Insert { text, replace } => {
                if let Some(range) = replace
                    && let Err(err) = self.replace_utf16(range)
                {
                    self.core.echo(err);
                    return;
                }
                for c in text.chars() {
                    self.key(Key::plain(KeyCode::Char(c)));
                }
            }
            // A window deleted since the frontend last drew is gone; drop its mouse events.
            Msg::Scroll { window, .. } | Msg::Click { window, .. }
                if !self.core.tree.leaves().contains(&window) => {}
            // At an end the wheel just stops, without C-v's message.
            Msg::Scroll { window, lines } => {
                let selected = self.core.selected;
                self.core.select(window);
                _ = self.scroll_lines(lines);
                self.core.select(selected);
            }
            Msg::Click { window, line, col } => {
                // shortcut: a click aborts the minibuffer, losing its text; keep it active once commands can leave it.
                self.core.close_minibuffer();
                self.core.select(window);
                self.click(line, col);
            }
            Msg::Resize {
                width,
                height,
                cell_width,
            } => self.core.frame_size = (width, height, cell_width),
            Msg::Saved {
                buffer,
                tick,
                result,
            } => self.on_saved(buffer, tick, result),
            Msg::Reverted {
                buffer,
                tick,
                result,
            } => self.core.on_reverted(buffer, tick, result),
            Msg::Diffed { buffer, result } => self.on_diffed(buffer, result),
            Msg::Listed { dir, result } => self.on_listed(dir, result),
            Msg::Opened { path, result } => {
                match result {
                    Ok(file) => self.core.visit(&path, file, true),
                    Err(err) => self.core.echo(err),
                }
                self.core.grep_jump(&path);
            }
            Msg::Previewed { path, result } => self.core.on_previewed(path, result),
            Msg::SettingsLoaded(text) => {
                match text.and_then(|text| Settings::parse(text.as_deref())) {
                    Ok(settings) => self.core.settings = settings,
                    Err(err) => self.core.report(format!("settings: {err}")),
                }
            }
            Msg::KeymapLoaded(text) => {
                match text.and_then(|text| self.core.parse_keymaps(text.as_deref())) {
                    Ok((keymaps, errors)) => {
                        self.core.keymaps = keymaps;
                        if !errors.is_empty() {
                            self.core.report(format!("keymap: {}", errors.join("; ")));
                        }
                    }
                    Err(err) => self.core.report(format!("keymap: {err}")),
                }
            }
            Msg::ProjectsLoaded { dir, projects } => {
                self.core.projects_dir = dir;
                // Ad-hoc projects stay, since workspaces are bound to them.
                self.core.projects.retain(|p| p.file.is_none());
                for project in projects {
                    match project {
                        Ok(project) => self.core.projects.push(project),
                        Err(err) => self.core.report(format!("project {err}")),
                    }
                }
                // A hand edit may have changed roots or ignore patterns; a project gone fails to walk.
                let indexed: Vec<String> = self.core.index.keys().cloned().collect();
                for name in indexed {
                    _ = self.index_project(&name, true);
                }
            }
            Msg::ProjectWritten(result) => self.core.project_written(result),
            Msg::Indexed {
                project,
                walk,
                roots,
            } => self.core.indexed(project, walk, roots),
            Msg::Grepped { search, hits, done } => self.core.on_grepped(search, hits, done),
            Msg::FilesChanged { paths, rescan } => self.on_files_changed(paths, rescan),
            Msg::Checked(files) => self.on_checked(files),
            Msg::Hint => {
                self.core.hint_timer = None;
                self.core.hint_shown = true;
            }
            Msg::Clipboard(text) => self.core.kills.clipboard = Some(text),
            Msg::Log(line) => self.core.log(&line),
            Msg::Quit => self.call("save-buffers-kill-terminal", Args::default()),
            Msg::AutoSave => self.write_autosaves(),
            Msg::Recover(files) if !files.is_empty() => {
                self.core.recovered.extend(files);
                self.call("recover-buffers", Args::default());
            }
            Msg::Recover(_) => {}
            Msg::Tick => {}
        }
    }

    /// Adds `key` to the pending sequence and runs what the keymaps bind it to; an unbound
    /// plain char runs `self-insert-command`.
    fn key(&mut self, key: Key) {
        let core = &mut *self.core;
        if let Some(keys) = &mut core.recording {
            keys.push(key);
        }
        core.message = None;
        if let Some(id) = core.hint_timer.take() {
            core.timers.cancel(id);
        }
        core.hint_shown = false;
        let point = core.window.point(&core.buffers);
        // Typed chars share an undo group, up to 20 like Emacs; any other key ends the run.
        let self_insert = matches!(key.code, KeyCode::Char(_))
            && !key.ctrl
            && !key.meta
            && core.pending.is_empty();
        if !(self_insert && (1..20).contains(&core.self_inserts)) {
            core.buffers
                .get_mut(core.window.buffer())
                .expect("current buffer is live")
                .undo_boundary(point);
            core.self_inserts = 0;
        }
        core.self_inserts += usize::from(self_insert);
        let quit = Key {
            ctrl: true,
            ..Key::plain(KeyCode::Char('g'))
        };
        if !core.pending.is_empty() && key == quit {
            core.pending.clear();
            core.transient = None;
            core.prefix_arg = None;
            core.echo("Quit".into());
            core.last_command = Some("keyboard-quit");
            return;
        }
        core.pending.push(key);
        let command = match core.lookup(&core.pending) {
            Some(Binding::Prefix(_)) => {
                core.hint_timer = Some(core.timers.start(self.now, HINT_DELAY, None, Msg::Hint));
                return;
            }
            Some(Binding::Command(name)) => Some(*name),
            // An unbound plain char still types, as #21 decided.
            None if self_insert => Some("self-insert-command"),
            None => None,
        };
        let keys = std::mem::take(&mut core.pending);
        core.transient = None;
        let prefix = core.prefix_arg.take();
        if let Some(name) = command {
            core.this_command = Some(name);
            core.this_keys = keys;
            let args = Args {
                prefix,
                ..Args::default()
            };
            self.call(name, args);
            let core = &mut *self.core;
            let this = core.this_command.take();
            // Prefix argument commands keep the last command, like prefix keys.
            if core.prefix_arg.is_none() {
                core.last_command = this;
            }
            return;
        }
        core.last_command = None;
        let keys: Vec<String> = keys.iter().map(Key::to_string).collect();
        core.echo(format!("{} is undefined", keys.join(" ")));
    }
}

pub struct Core {
    buffers: Buffers,
    /// The selected window, showing the current buffer.
    window: Window,
    selected: WindowId,
    /// Every window but the selected one.
    others: HashMap<WindowId, Window>,
    tree: Tree,
    /// Every workspace, in creation order; `buffers.workspace` is the current one.
    workspaces: Vec<Workspace>,
    /// The id the next new workspace gets.
    next_workspace: u64,
    /// How many times a workspace was shown, for `Workspace::shown`.
    shown: u64,
    /// The windows area's width and height and the cell width in px, as the frontend reported.
    frame_size: (f32, f32, f32),
    /// The id the next new window gets.
    next_window: usize,
    timers: Timers,
    /// Keymaps by mode name, plus `global`.
    keymaps: HashMap<String, Keymap>,
    /// Looked up before every other keymap, for the next key sequence only.
    transient: Option<Keymap>,
    /// The keys of a sequence that is still a prefix.
    pending: Vec<Key>,
    /// The timer that shows the hint for `pending`.
    hint_timer: Option<TimerId>,
    /// Whether the hint for `pending` is shown.
    hint_shown: bool,
    /// The command the current key sequence runs, while it runs.
    this_command: Option<&'static str>,
    /// The command the previous complete key sequence ran; `None` if it ran none.
    last_command: Option<&'static str>,
    /// The prefix argument being built, for the next command.
    prefix_arg: Option<Prefix>,
    /// The keys of the sequence that ran the current or last command.
    this_keys: Vec<Key>,
    /// Every key received since recording started, for keyboard macros.
    recording: Option<Vec<Key>>,
    /// Typed chars in the current undo group, for amalgamation.
    self_inserts: usize,
    prompts: Prompts,
    /// The column a run of line motions aims for.
    goal_column: usize,
    /// Where the last `recenter-top-bottom` put point's line: middle, top or bottom.
    recenter: usize,
    kills: KillRing,
    settings: Settings,
    projects: Vec<Loaded>,
    /// Where new project files go; `None` without a config dir.
    projects_dir: Option<PathBuf>,
    /// Each indexed project's files, by project name.
    index: HashMap<String, FileIndex>,
    /// How many `FileOp::Index` walks were asked for; numbers each one.
    index_walks: u64,
    grep: grep::ProjectGrep,
    commands: HashMap<&'static str, Command>,
    /// shortcut: one line for settings errors, cleared on the next key; the echo area (#36) replaces it.
    message: Option<String>,
    /// The buffers whose saves a quit, or the kill of `then_kill`, waits for; empty when not
    /// quitting.
    quitting: Vec<BufferId>,
    /// The workspace to kill rather than quit once the saves in `quitting` succeed.
    then_kill: Option<WorkspaceId>,
    /// Buffers with a recovery file, by the tick written to it.
    autosaved: HashMap<BufferId, u64>,
    /// The auto-save timers: restarted by each edit, and started by the first edit after a write.
    autosave_idle: Option<TimerId>,
    autosave_max: Option<TimerId>,
    /// Earlier sessions' recovery files not yet recovered or discarded, oldest first.
    recovered: Vec<Recovered>,
    /// The watch set last sent to the watcher.
    watching: WatchSet,
    /// The selected window's buffer after the last message handled with no minibuffer open,
    /// for `ask_on_focus`.
    focused: BufferId,
}

impl Default for Core {
    fn default() -> Self {
        // The launch workspace's project until `launch` names another.
        let home = std::env::home_dir().unwrap_or_else(|| "/".into());
        let name = buffer::file_name(&home);
        let mut buffers = Buffers::default();
        let scratch = buffers.create_scratch(&name);
        let window = Window::new(&mut buffers, scratch);
        let mut commands = HashMap::new();
        for c in builtins::builtins() {
            assert!(
                commands.insert(c.name, c).is_none(),
                "duplicate builtin {}",
                c.name
            );
        }
        let mut core = Core {
            buffers,
            window,
            selected: WindowId(0),
            others: HashMap::new(),
            tree: Tree::Leaf(WindowId(0)),
            workspaces: vec![Workspace::new(WorkspaceId::default(), &name, &name)],
            next_workspace: 1,
            shown: 0,
            frame_size: (0., 0., 0.),
            next_window: 1,
            timers: Timers::default(),
            keymaps: HashMap::new(),
            transient: None,
            pending: Vec::new(),
            hint_timer: None,
            hint_shown: false,
            this_command: None,
            last_command: None,
            prefix_arg: None,
            this_keys: Vec::new(),
            recording: None,
            self_inserts: 0,
            prompts: Prompts::default(),
            goal_column: 0,
            recenter: 0,
            kills: KillRing::default(),
            settings: Settings::default(),
            projects: vec![Loaded::ad_hoc(&name, &home)],
            projects_dir: None,
            index: HashMap::new(),
            index_walks: 0,
            grep: grep::ProjectGrep::default(),
            commands,
            message: None,
            quitting: Vec::new(),
            then_kill: None,
            autosaved: HashMap::new(),
            autosave_idle: None,
            autosave_max: None,
            recovered: Vec::new(),
            watching: WatchSet::default(),
            focused: scratch,
        };
        let (keymaps, errors) = core.parse_keymaps(None).expect("bundled keymap parses");
        assert!(errors.is_empty(), "bundled keymap: {errors:?}");
        core.keymaps = keymaps;
        core
    }
}

impl Core {
    /// Shows `msg` in the echo area and logs it to `*Messages*`.
    fn echo(&mut self, msg: String) {
        self.log(&msg);
        self.message = Some(msg);
    }

    /// Appends `line` to `*Messages*`, making the buffer if there is none.
    fn log(&mut self, line: &str) {
        let id = self.buffers.find("*Messages*").unwrap_or_else(|| {
            let id = self.buffers.create("*Messages*");
            let buffer = self.buffers.get_mut(id).expect("just created");
            (buffer.read_only, buffer.unkillable, buffer.workspace) = (true, true, None);
            id
        });
        let buffer = self.buffers.get_mut(id).expect("found or created");
        buffer.append(&format!("{line}\n"));
    }

    /// Shows `msg` on the message line, after what a load already put there this key.
    fn report(&mut self, msg: String) {
        self.log(&msg);
        self.message = Some(match self.message.take() {
            Some(old) => format!("{old}; {msg}"),
            None => msg,
        });
    }

    /// Handles `msg` at `now`, then fires the timers due by then.
    pub fn handle(&mut self, now: Instant, msg: Msg) -> Effects {
        let input = matches!(msg, Msg::Key(_) | Msg::Insert { .. });
        let mut ctx = Ctx {
            core: self,
            now,
            effects: Vec::new(),
        };
        ctx.dispatch(msg);
        for id in ctx.core.timers.due(now) {
            if let Some(msg) = ctx.core.timers.fire(id, now) {
                ctx.dispatch(msg);
            }
        }
        ctx.ask_on_focus();
        let mut effects = ctx.effects;
        self.autosave(now, input, &mut effects);
        self.poll_completion(now, &mut effects);
        self.preview(&mut effects);
        self.watch(&mut effects);
        effects
    }

    /// Adds `command`, replacing any command of the same name.
    pub fn register(&mut self, command: Command) {
        self.commands.insert(command.name, command);
    }

    /// When the app must wake the core with `Msg::Tick`, if any timer is live.
    pub fn next_deadline(&self) -> Option<Instant> {
        self.timers.next_deadline()
    }

    pub fn current(&self) -> &Buffer {
        self.buffers
            .get(self.window.buffer())
            .expect("current buffer is live")
    }

    fn current_mut(&mut self) -> &mut Buffer {
        self.buffers
            .get_mut(self.window.buffer())
            .expect("current buffer is live")
    }

    pub fn text(&self) -> String {
        self.current().text().to_string()
    }

    pub fn settings(&self) -> &Settings {
        &self.settings
    }

    pub fn projects(&self) -> Vec<&Project> {
        self.projects.iter().map(|p| &p.project).collect()
    }

    pub fn this_command(&self) -> Option<&'static str> {
        self.this_command
    }

    pub fn last_command(&self) -> Option<&'static str> {
        self.last_command
    }

    /// Starts keeping every key received, dropping any keys kept so far.
    pub fn start_recording(&mut self) {
        self.recording = Some(Vec::new());
    }

    /// Stops recording and returns the keys kept, or `None` if not recording.
    pub fn take_recording(&mut self) -> Option<Vec<Key>> {
        self.recording.take()
    }

    pub fn message(&self) -> Option<&str> {
        self.message.as_deref()
    }

    /// Point as a char offset into the buffer.
    pub fn point(&self) -> usize {
        self.window.point(&self.buffers)
    }
}

#[cfg(test)]
mod tests {
    use crate::Msg;
    use crate::command::ArgSpec;
    use crate::command::Args;
    use crate::command::Command;
    use crate::command::Prefix;
    use crate::harness::Harness;
    use crate::harness::MS;
    use crate::harness::bind_mode;
    use crate::harness::insert_cmd;
    use crate::harness::msg;
    use crate::harness::name;
    use crate::harness::on;
    use crate::harness::test_cmd;
    use crate::harness::twenty_lines;
    use crate::key::parse_keys;
    use crate::window::WindowId;

    #[test]
    fn calls_a_command_by_name_with_its_args() {
        let mut h = Harness::new(&[]);
        test_cmd(
            &mut h,
            ArgSpec {
                region: false,
                prompts: &["Text: "],
                ..ArgSpec::default()
            },
        );
        let args = Args {
            prefix: Some(Prefix::Number(3)),
            strings: vec!["hello".into()],
            ..Args::default()
        };
        h.call("test-cmd", args.clone());
        assert_eq!(h.core.message(), Some(format!("{args:?}").as_str()));
        h.core.message = None;
        h.call("test-cmd", Args::default());
        assert_eq!(
            h.core.message(),
            None,
            "the handler waits for the minibuffer"
        );
        assert_eq!(h.core.minibuffer().unwrap().prompt, "Text: ");
        h.keys("C-g");
        h.call("no-such-command", Args::default());
        assert_eq!(h.core.message(), Some("No command named no-such-command"));
    }

    #[test]
    fn region_comes_from_the_caller_or_point_and_mark() {
        let mut h = Harness::new(&[]);
        test_cmd(
            &mut h,
            ArgSpec {
                region: true,
                prompts: &[],
                ..ArgSpec::default()
            },
        );
        h.keys("abcde <left>");
        h.call("test-cmd", Args::default());
        assert_eq!(
            h.core.message(),
            Some("The mark is not set now, so there is no region")
        );
        let given = Args {
            region: Some(0..2),
            ..Args::default()
        };
        h.call("test-cmd", given.clone());
        assert_eq!(h.core.message(), Some(format!("{given:?}").as_str()));
        h.core.window.set_mark(&mut h.core.buffers, 1).unwrap();
        h.call("test-cmd", Args::default());
        assert_eq!(
            h.core.message(),
            Some("Args { prefix: None, region: Some(1..4), strings: [] }")
        );
    }

    #[test]
    fn handler_errors_go_to_the_message_line() {
        let mut h = Harness::new(&[]);
        h.core.register(Command {
            name: "fail",
            spec: ArgSpec::default(),
            handler: |_, _| Err("boom".into()),
        });
        h.call("fail", Args::default());
        assert_eq!(h.core.message(), Some("boom"));
    }

    #[test]
    fn a_panicking_command_is_logged_and_the_editor_keeps_running() {
        let mut h = on("");
        h.core.register(Command {
            name: "explode",
            spec: ArgSpec::default(),
            handler: |_, _| panic!("boom"),
        });
        h.keys("M-x explode RET");
        let msg = "explode panicked; its state may be inconsistent (see *Messages*)";
        assert_eq!(h.core.message(), Some(msg));
        let log = messages(&h).unwrap();
        assert!(log.contains(msg) && log.contains("boom"), "{log}");
        assert!(log.contains("harness.rs:"), "no location: {log}");
        assert!(log.contains("try_call"), "no backtrace: {log}");
        h.keys("a");
        assert_eq!(h.text(), "a");
    }

    #[test]
    fn command_errors_show_in_the_frame_until_the_next_key() {
        let mut h = on("");
        h.core.register(Command {
            name: "fail",
            spec: ArgSpec::default(),
            handler: |_, _| Err("boom".into()),
        });
        h.call("fail", Args::default());
        insta::assert_debug_snapshot!(h.frame(800., 600.));
        h.keys("a");
        assert_eq!(h.frame(800., 600.).message, None);
    }

    #[test]
    fn echo_area_clears_on_click_not_on_scroll_or_timers() {
        let mut h = twenty_lines();
        h.keys("C-g");
        h.send(Msg::Scroll {
            window: WindowId(0),
            lines: 2,
        });
        h.start_timer(MS, None, Msg::Tick);
        h.advance(2 * MS);
        assert_eq!(h.core.message(), Some("Quit"));
        h.keys("M-z");
        assert_eq!(h.core.message(), Some("M-z is undefined"));
        h.send(Msg::Click {
            window: WindowId(0),
            line: 0,
            col: 0,
        });
        assert_eq!(h.core.message(), None);
    }

    #[test]
    fn one_shot_fires_once_at_its_deadline() {
        let mut h = Harness::new(&[]);
        h.start_timer(100 * MS, None, msg('a'));
        h.advance(99 * MS);
        assert_eq!(h.text(), "");
        h.advance(MS).advance(100 * MS);
        assert_eq!(h.text(), "a");
        assert_eq!(h.core.next_deadline(), None);
    }

    #[test]
    fn repeat_fires_each_interval_until_cancelled() {
        let mut h = Harness::new(&[]);
        let id = h.start_timer(100 * MS, Some(100 * MS), msg('a'));
        h.advance(100 * MS).advance(100 * MS).advance(100 * MS);
        assert_eq!(h.text(), "aaa");
        h.cancel_timer(id);
        h.advance(100 * MS);
        assert_eq!(h.text(), "aaa");
    }

    #[test]
    fn late_wake_up_does_not_catch_up() {
        let mut h = Harness::new(&[]);
        h.start_timer(100 * MS, Some(100 * MS), msg('a'));
        h.advance(350 * MS);
        assert_eq!(h.text(), "a");
    }

    #[test]
    fn cancelled_before_deadline_never_fires() {
        let mut h = Harness::new(&[]);
        let id = h.start_timer(100 * MS, None, msg('a'));
        h.cancel_timer(id);
        h.advance(200 * MS);
        assert_eq!(h.text(), "");
    }

    #[test]
    fn due_timers_fire_on_any_message() {
        let mut h = Harness::new(&[]);
        h.start_timer(100 * MS, None, msg('a'));
        h.now += 100 * MS;
        h.keys("b");
        assert_eq!(h.text(), "ba");
    }

    #[test]
    fn next_deadline_is_the_earliest_timer() {
        let mut h = Harness::new(&[]);
        let t0 = h.now;
        h.start_timer(300 * MS, None, msg('a'));
        h.start_timer(100 * MS, None, msg('b'));
        assert_eq!(h.core.next_deadline(), Some(t0 + 100 * MS));
    }

    #[test]
    fn key_sequences_run_commands_and_track_the_last_one() {
        let mut h = Harness::new(&[]);
        insert_cmd(&mut h);
        h.keys("C-c t");
        assert_eq!(h.text(), "hi");
        assert_eq!(h.core.message(), Some("Some(\"test-insert\") None"));
        h.keys("C-c t");
        assert_eq!(h.text(), "hihi");
        assert_eq!(
            h.core.message(),
            Some("Some(\"test-insert\") Some(\"test-insert\")")
        );
        assert_eq!(h.core.this_command(), None);
        h.keys("C-x");
        assert_eq!(h.core.last_command(), Some("test-insert"));
        h.keys("C-g");
        assert_eq!(h.core.last_command(), Some("keyboard-quit"));
        h.keys("C-c t a");
        assert_eq!(h.core.last_command(), Some("self-insert-command"));
        h.keys("C-c t C-x C-q");
        assert_eq!(h.core.last_command(), None);
    }

    #[test]
    fn nested_calls_leave_command_tracking_alone() {
        let mut h = Harness::new(&[]);
        h.core.register(Command {
            name: "test-nested",
            spec: ArgSpec::default(),
            handler: |ctx, _| {
                ctx.call("undo", Args::default());
                Ok(())
            },
        });
        bind_mode(&mut h, "global", "C-c n", "test-nested");
        h.keys("a C-c n");
        assert_eq!(h.text(), "");
        assert_eq!(h.core.last_command(), Some("test-nested"));
    }

    #[test]
    fn records_the_key_stream_until_taken() {
        let mut h = Harness::new(&[]);
        h.keys("x");
        assert_eq!(h.core.take_recording(), None);
        h.core.start_recording();
        h.keys("a C-x C-g b");
        assert_eq!(
            h.core.take_recording(),
            Some(parse_keys("a C-x C-g b").unwrap())
        );
        h.keys("c");
        assert_eq!(h.core.take_recording(), None);
    }

    fn messages(h: &Harness) -> Option<String> {
        let id = h.core.buffers.find("*Messages*")?;
        Some(h.core.buffers.get(id).unwrap().text().to_string())
    }

    #[test]
    fn echo_messages_and_log_lines_go_to_messages() {
        let mut h = Harness::new(&[]);
        assert_eq!(messages(&h), None);
        h.keys("C-g C-g M-z");
        h.send(Msg::Log("WARN aiditor: careful".into()));
        assert_eq!(
            messages(&h).unwrap(),
            "Quit\nQuit\nM-z is undefined\nWARN aiditor: careful\n"
        );
        // Each part of a joined message gets its own line.
        h.send(Msg::SettingsLoaded(Err("e1".into())));
        h.send(Msg::KeymapLoaded(Err("e2".into())));
        assert!(
            messages(&h)
                .unwrap()
                .ends_with("careful\nsettings: e1\nkeymap: e2\n")
        );
    }

    #[test]
    fn projects_load_and_their_errors_share_the_echo_line() {
        let mut h = Harness::new(&[]);
        let home = h.core.projects()[0].clone();
        let ok = crate::Project::parse("name = \"a\"\n[primary]\npath = \"/r/a\"").unwrap();
        // Projects may share a root.
        let shared = crate::Project::parse(
            "name = \"d\"\n[primary]\npath = \"/r/d\"\n[[roots]]\npath = \"/r/a\"",
        )
        .unwrap();
        use crate::project::{Loaded, Project};
        let loaded = |project: &Project| Loaded {
            project: project.clone(),
            file: Some(("/p.toml".into(), String::new())),
        };
        h.send(Msg::ProjectsLoaded {
            dir: None,
            projects: vec![
                Err("b.toml: /r/b does not exist".into()),
                Ok(loaded(&ok)),
                Err("c.toml: name is empty".into()),
                Ok(loaded(&shared)),
            ],
        });
        assert_eq!(h.core.projects(), [&home, &ok, &shared]);
        assert_eq!(
            h.core.message(),
            Some("project b.toml: /r/b does not exist; project c.toml: name is empty")
        );
        // A reload replaces the saved projects and keeps the ad-hoc ones.
        h.send(Msg::ProjectsLoaded {
            dir: None,
            projects: Vec::new(),
        });
        assert_eq!(h.core.projects(), [&home]);
    }

    #[test]
    fn messages_is_read_only_and_unkillable() {
        let mut h = Harness::new(&[]);
        h.keys("C-g C-x b *Messages* RET a");
        assert_eq!(name(&h), "*Messages*");
        assert_eq!(h.core.message(), Some("Buffer is read-only"));
        assert_eq!(h.text(), "Quit\nBuffer is read-only\n");
        h.keys("C-x k RET");
        assert_eq!(h.core.message(), Some("Can't kill *Messages*"));
        assert_eq!(name(&h), "*Messages*");
    }
}
