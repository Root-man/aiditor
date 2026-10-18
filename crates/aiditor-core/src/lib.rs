//! Editor state. A plain state machine with no GPUI or Tokio dependency.

pub mod buffer;
pub mod command;
pub mod display;
#[cfg(test)]
mod harness;
pub mod key;
pub mod keymap;
pub mod settings;
pub mod timer;
pub mod window;

use std::collections::{HashMap, VecDeque};
use std::ops::Range;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use nucleo::Nucleo;
use nucleo::pattern::{CaseMatching, Normalization};
use ropey::Rope;

use buffer::{Buffer, BufferId, Buffers};
use command::{ArgSpec, Args, Command, Prefix};
use key::{Key, KeyCode};
use keymap::{Binding, Keymap};
use settings::Settings;
use timer::{TimerId, Timers};
use window::{Tree, Window, WindowId};

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
    /// The file service's answer to `Effect::Save`: the file's new hash, or why it wasn't written.
    Saved {
        buffer: BufferId,
        tick: u64,
        result: Result<u64, String>,
    },
    /// The file service's answer to `Effect::ListDir`: the directory's entries, or why it
    /// couldn't be listed.
    Listed {
        dir: PathBuf,
        result: Result<Vec<String>, String>,
    },
    /// The file service's answer to `Effect::Open`: the file's bytes and hash, `None` when there
    /// is no file, or why it couldn't be read.
    Opened {
        path: PathBuf,
        result: Result<Option<(Vec<u8>, u64)>, String>,
    },
    /// The user's settings file: its text, `None` if it doesn't exist, or why it couldn't be read.
    SettingsLoaded(Result<Option<String>, String>),
    /// The user's keymap file, the same way.
    KeymapLoaded(Result<Option<String>, String>),
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
}

/// How long after the last edit buffers are auto-saved, and the most edits wait while typing goes on.
const AUTOSAVE_IDLE: Duration = Duration::from_secs(2);
const AUTOSAVE_MAX: Duration = Duration::from_secs(30);

/// How long a prefix waits before the hint shows, Emacs's `echo-keystrokes`.
const HINT_DELAY: Duration = Duration::from_secs(1);

/// How many entries the kill ring keeps, Emacs's `kill-ring-max`.
const KILL_RING_MAX: usize = 60;

/// How many answers each prompt's history keeps, Emacs's `history-length`.
const HISTORY_MAX: usize = 100;

/// How many candidates the completion list shows, Vertico's `vertico-count`.
const COMPLETION_ROWS: usize = 10;

/// How long the core waits for the matcher before trying again on a timer.
const REPOLL: Duration = Duration::from_millis(16);

/// The minibuffer's window, kept outside the split tree.
const MINIBUFFER: WindowId = WindowId(usize::MAX);

/// A prompt waiting in the minibuffer for the command that asked.
struct Minibuffer {
    command: &'static str,
    /// The args so far; the answer becomes the next string.
    args: Args,
    prompt: String,
    /// The window to select again on closing.
    from: WindowId,
    /// The history entry shown, 1 the newest; 0 is the text typed before moving.
    index: usize,
    /// What was typed before the first history move.
    input: String,
    /// The completion list, when the command completes over candidates.
    completion: Option<Completion>,
}

/// The selection in a completing minibuffer's list of matches.
struct Completion {
    /// The text the matcher's pattern was last set from.
    pattern: String,
    /// The selected match, best first.
    selected: usize,
    /// The first match shown, so the shown rows hold the selection.
    top: usize,
    /// For file names, the text through its last `/`: the directory listed or awaited.
    dir: Option<String>,
    /// Whether `dir`'s listing arrived, so a second answer is dropped.
    listed: bool,
    /// The directory a relative `dir` is listed from, as `find-file` visits from it.
    base: PathBuf,
}

impl Completion {
    /// Selects match `selected`, scrolling the shown rows only as far as needed.
    fn select(&mut self, selected: usize) {
        self.selected = selected;
        self.top = self
            .top
            .min(selected)
            .max((selected + 1).saturating_sub(COMPLETION_ROWS));
    }
}

/// Work the app carries out for the core.
pub enum Effect {
    /// Write `bytes` to `path` if the file still hashes to `hash` (`None`: no file), then send
    /// `Msg::Saved` with `buffer` and its `tick` at the save.
    Save {
        buffer: BufferId,
        tick: u64,
        path: PathBuf,
        bytes: Vec<u8>,
        hash: Option<u64>,
    },
    /// List the directory and send `Msg::Listed`.
    ListDir(PathBuf),
    /// Read the file and send `Msg::Opened`.
    Open(PathBuf),
    /// Read the user's settings and keymap files and send `Msg::SettingsLoaded` and
    /// `Msg::KeymapLoaded`.
    LoadSettings,
    /// Put this text on the system clipboard.
    SetClipboard(String),
    /// Quit the app.
    Quit,
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
            return self.read_string(command, args, prompt.into());
        }
        (command.handler)(self, args)
    }

    /// Opens the minibuffer at `prompt`; its answer then calls `command` again.
    fn read_string(&mut self, command: Command, args: Args, prompt: String) -> Result<(), String> {
        let core = &mut *self.core;
        if core.minibuffer.is_some() {
            return Err("Command attempted to use minibuffer while in minibuffer".into());
        }
        let base = core.directory();
        let start = (command.spec.files).then(|| base.join("").to_string_lossy().into_owned());
        let candidates = command
            .spec
            .completion
            .map(|candidates| candidates(core, &args));
        let completion = (candidates.is_some() || start.is_some()).then(|| {
            // One matcher for the core, as each one starts a thread pool.
            let matcher = core.matcher.get_or_insert_with(|| {
                Nucleo::new(nucleo::Config::DEFAULT, Arc::new(|| {}), None, 1)
            });
            matcher.restart(true);
            (matcher.pattern).reparse(0, "", CaseMatching::Smart, Normalization::Smart, false);
            let injector = matcher.injector();
            for c in candidates.into_iter().flatten() {
                let note = command.spec.annotation.and_then(|f| f(core, &c));
                injector.push((c, note.unwrap_or_default()), |(c, _), cols| {
                    cols[0] = c.as_str().into()
                });
            }
            Completion {
                pattern: String::new(),
                selected: 0,
                top: 0,
                // Nothing listed yet; `poll_completion` asks for the start directory.
                dir: command.spec.files.then(String::new),
                listed: false,
                base,
            }
        });
        // A fresh buffer per prompt, so undo and markers never carry over.
        let id = core.buffers.create(" *Minibuf-1*");
        let buffer = core.buffers.get_mut(id).expect("just created");
        buffer.major_mode = "minibuffer-mode";
        if completion.is_some() {
            buffer.minor_modes.push("minibuffer-completion");
        }
        let window = Window::new(&mut core.buffers, id);
        core.others.insert(MINIBUFFER, window);
        core.minibuffer = Some(Minibuffer {
            command: command.name,
            args,
            prompt,
            from: core.selected,
            index: 0,
            input: String::new(),
            completion,
        });
        core.select(MINIBUFFER);
        match start {
            Some(text) => self.set_text(&text),
            None => Ok(()),
        }
    }

    /// Closes the minibuffer and calls its command with the selected candidate, or with `input`
    /// or no match the text, as the next string. A selected directory is entered instead.
    fn exit_minibuffer(&mut self, input: bool) -> Result<(), String> {
        let selected = (!input).then(|| self.core.selected_candidate()).flatten();
        let completion = self
            .core
            .minibuffer
            .as_ref()
            .and_then(|mb| mb.completion.as_ref());
        if completion.is_some_and(|c| c.dir.is_some())
            && let Some(dir) = selected.as_ref().filter(|s| s.ends_with('/'))
        {
            return self.set_text(dir);
        }
        let text = selected.unwrap_or_else(|| self.core.text());
        let mb = self.core.close_minibuffer().ok_or("Not in a minibuffer")?;
        let history = self.core.history.entry(mb.prompt).or_default();
        if !text.is_empty() && history.first() != Some(&text) {
            history.insert(0, text.clone());
            history.truncate(HISTORY_MAX);
        }
        let mut args = mb.args;
        args.strings.push(text);
        self.core.this_command = Some(mb.command);
        self.call(mb.command, args);
        Ok(())
    }

    /// Replaces the minibuffer's text with the history entry `n` older, newer when negative.
    fn history_element(&mut self, n: i64) -> Result<(), String> {
        let text = self.core.text();
        let core = &mut *self.core;
        let mb = core.minibuffer.as_mut().ok_or("Not in a minibuffer")?;
        let items = core.history.get(&mb.prompt).map_or(&[][..], Vec::as_slice);
        let target = (mb.index as i64).saturating_add(n);
        if target > items.len() as i64 {
            return Err("Beginning of history; no preceding item".into());
        }
        if target < 0 {
            return Err("End of history; no default available".into());
        }
        if mb.index == 0 {
            mb.input = text;
        }
        mb.index = target as usize;
        let new = match mb.index {
            0 => mb.input.clone(),
            i => items[i - 1].clone(),
        };
        self.set_text(&new)
    }

    /// Replaces the current buffer's text with `new`, point at its end.
    fn set_text(&mut self, new: &str) -> Result<(), String> {
        let buffer = self.core.current_mut();
        let len = buffer.len_chars();
        buffer.delete(0..len).map_err(|err| err.to_string())?;
        buffer.insert(0, new).map_err(|err| err.to_string())?;
        self.goto(new.chars().count())
    }

    /// Moves the completion selection `n` matches down, up when negative, stopping at the ends.
    fn next_completion(&mut self, n: i64) -> Result<(), String> {
        let count = self.core.matches();
        let completion = (self.core.minibuffer.as_mut())
            .and_then(|mb| mb.completion.as_mut())
            .ok_or("Not completing")?;
        if count == 0 {
            return Err("No match".into());
        }
        let target = (completion.selected as i64).saturating_add(n);
        completion.select(target.clamp(0, count as i64 - 1) as usize);
        Ok(())
    }

    /// Fails with `err` while the minibuffer is selected.
    fn outside_minibuffer(&self, err: &str) -> Result<(), String> {
        match self.core.selected == MINIBUFFER {
            true => Err(err.into()),
            false => Ok(()),
        }
    }

    fn dispatch(&mut self, msg: Msg) {
        match msg {
            Msg::Key(key) => {
                self.key(key);
                // The clipboard text was for this key only.
                self.core.clipboard = None;
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
                buffer: id,
                tick,
                result,
            } => {
                let ok = result.is_ok();
                match (self.core.buffers.get_mut(id), result) {
                    (None, _) => {}
                    (Some(buffer), Ok(hash)) => {
                        buffer.disk = Some(hash);
                        // Edits made while the write ran are still unsaved.
                        if buffer.tick() == tick {
                            buffer.set_saved();
                        }
                        let path = buffer.file.as_deref().unwrap_or(Path::new(""));
                        let msg = format!("Wrote {}", path.display());
                        self.core.echo(msg);
                    }
                    (Some(_), Err(err)) => self.core.echo(err),
                }
                let core = &mut *self.core;
                if let Some(at) = core.quitting.iter().position(|&b| b == id) {
                    // A failed save, or edits made while it ran, cancel the quit.
                    if ok && !core.buffers.get(id).is_some_and(Buffer::modified) {
                        core.quitting.swap_remove(at);
                        if core.quitting.is_empty() {
                            self.effects.push(Effect::Quit);
                        }
                    } else {
                        core.quitting.clear();
                    }
                }
            }
            Msg::Listed { dir, result } => {
                let core = &mut *self.core;
                let awaited = (core.minibuffer.as_mut())
                    .and_then(|mb| mb.completion.as_mut())
                    .filter(|c| {
                        !c.listed && c.dir.as_ref().map(|d| c.base.join(d)) == Some(dir.clone())
                    });
                let Some(completion) = awaited else {
                    return;
                };
                completion.listed = true;
                match result {
                    Ok(names) => {
                        let matcher = core.matcher.as_ref().expect("completing has a matcher");
                        let injector = matcher.injector();
                        for name in names {
                            injector.push((name, String::new()), |(c, _), cols| {
                                cols[0] = c.as_str().into()
                            });
                        }
                    }
                    Err(err) => core.echo(err),
                }
            }
            Msg::Opened { path, result } => match result {
                Ok(file) => self.core.visit(&path, file, true),
                Err(err) => self.core.echo(err),
            },
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
            Msg::Hint => {
                self.core.hint_timer = None;
                self.core.hint_shown = true;
            }
            Msg::Clipboard(text) => self.core.clipboard = Some(text),
            Msg::Log(line) => self.core.log(&line),
            Msg::Quit => self.call("save-buffers-kill-terminal", Args::default()),
            Msg::AutoSave => {
                let core = &mut *self.core;
                for id in [core.autosave_idle.take(), core.autosave_max.take()]
                    .into_iter()
                    .flatten()
                {
                    core.timers.cancel(id);
                }
                for (id, b) in core.buffers.recent() {
                    if autosaves(b) && core.autosaved.get(&id) != Some(&b.tick()) {
                        core.autosaved.insert(id, b.tick());
                        self.effects.push(Effect::AutoSave {
                            buffer: id,
                            name: b.name.clone(),
                            path: b.file.clone(),
                            hash: b.disk,
                            text: b.text().clone(),
                        });
                    }
                }
            }
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

    /// Pushes clipboard text the editor neither wrote nor took in onto the kill ring, saying
    /// whether it did.
    fn take_clipboard(&mut self) -> bool {
        let core = &mut *self.core;
        let Some(text) = core.clipboard.take() else {
            return false;
        };
        if text.is_empty() || core.clipboard_seen.as_ref() == Some(&text) {
            return false;
        }
        core.clipboard_seen = Some(text.clone());
        core.kill_new(text);
        true
    }

    /// Saves the text in `range` on the kill ring, deleting it unless `copy`. Right after a
    /// kill it joins the newest entry, in front of it when `backward`.
    fn kill(&mut self, range: Range<usize>, backward: bool, copy: bool) -> Result<(), String> {
        let pushed = self.take_clipboard();
        let core = &mut *self.core;
        let text = (core.current().text().get_slice(range.clone()))
            .ok_or_else(|| format!("Region {range:?} is outside the buffer"))?
            .to_string();
        if !copy {
            core.current_mut()
                .delete(range)
                .map_err(|err| err.to_string())?;
        }
        match core.kill_ring.front_mut() {
            Some(head) if !pushed && core.last_command == Some("kill-region") => {
                if backward {
                    head.insert_str(0, &text);
                } else {
                    head.push_str(&text);
                }
                core.yank_index = 0;
            }
            _ => core.kill_new(text),
        }
        let head = core.kill_ring[0].clone();
        core.clipboard_seen = Some(head.clone());
        self.effects.push(Effect::SetClipboard(head));
        // Every kill counts as `kill-region` so the next one joins it, as in Emacs.
        if !copy {
            core.this_command = Some("kill-region");
        }
        Ok(())
    }

    /// Kills between point and `target`.
    fn kill_to(&mut self, target: usize) -> Result<(), String> {
        let point = self.core.point();
        self.kill(point.min(target)..point.max(target), target < point, false)
    }

    /// Kills, or with `copy` copies, the region; backward when point is before the mark.
    fn kill_region(&mut self, region: Option<Range<usize>>, copy: bool) -> Result<(), String> {
        let region = region.expect("the spec asks for the region");
        let backward = self.core.point() == region.start && !region.is_empty();
        self.kill(region, backward, copy)?;
        self.core.window.set_active(false);
        Ok(())
    }

    /// Kills to the end of the line, through the newline if only blanks are left; a prefix N
    /// kills N lines, or back to the start of the line N up when N is 0 or less.
    fn kill_line(&mut self, prefix: Option<Prefix>) -> Result<(), String> {
        let buffer = self.core.current();
        let point = self.core.point();
        let len = buffer.len_chars();
        let last = buffer.len_lines() as i64 - 1;
        let (line, _) = buffer.line_col(point).expect("point is in the buffer");
        let line_start = |l: i64| match l {
            l if l > last => len,
            l => (buffer.char_idx(l.max(0) as usize, 0)).expect("line is in the buffer"),
        };
        let target = match prefix {
            Some(p) => line_start((line as i64).saturating_add(p.value())),
            None if point == len => return Err("End of buffer".into()),
            None => {
                let end = (buffer.char_idx(line, usize::MAX)).expect("line is in the buffer");
                let blank = (point..end).all(|i| matches!(buffer.text().char(i), ' ' | '\t'));
                if blank {
                    line_start(line as i64 + 1)
                } else {
                    end
                }
            }
        };
        self.kill_to(target)
    }

    /// Inserts the kill `n` on from the yank pointer, with the mark at its start; a plain `C-u`
    /// leaves point at the start instead.
    fn yank(&mut self, prefix: Option<Prefix>) -> Result<(), String> {
        self.take_clipboard();
        let n = match prefix {
            None | Some(Prefix::Universal(_)) => 0,
            Some(Prefix::Minus) => -2,
            Some(p) => p.value().saturating_sub(1),
        };
        let text = self.core.current_kill(n)?;
        let core = &mut *self.core;
        let start = core.point();
        core.window
            .push_mark(&mut core.buffers, start, false)
            .map_err(|err| err.to_string())?;
        self.insert_n(&text, 1, false)?;
        if matches!(prefix, Some(Prefix::Universal(_))) {
            self.point_before_yank(start)?;
        }
        Ok(())
    }

    /// Replaces the text just yanked with the kill `n` further round the ring.
    fn yank_pop(&mut self, n: i64) -> Result<(), String> {
        if self.core.last_command != Some("yank") {
            return Err("Previous command was not a yank".into());
        }
        let text = self.core.current_kill(n)?;
        let (mark, point) = (self.mark()?, self.core.point());
        let start = point.min(mark);
        self.core
            .current_mut()
            .delete(start..point.max(mark))
            .map_err(|err| err.to_string())?;
        self.insert_n(&text, 1, false)?;
        if point < mark {
            self.point_before_yank(start)?;
        }
        self.core.this_command = Some("yank");
        Ok(())
    }

    /// Moves the mark to point at the yanked text's end and point to its `start`.
    fn point_before_yank(&mut self, start: usize) -> Result<(), String> {
        let core = &mut *self.core;
        let end = core.point();
        core.window
            .set_mark(&mut core.buffers, end)
            .map_err(|err| err.to_string())?;
        self.goto(start)
    }

    /// Deletes the UTF-16 `range` of the current buffer and puts point at its start.
    fn replace_utf16(&mut self, range: Range<usize>) -> Result<(), String> {
        let text = self.core.current().text();
        if range.start > range.end || range.end > text.len_utf16_cu() {
            return Err(format!(
                "Input method range {range:?} is outside the buffer"
            ));
        }
        let (start, end) = (
            text.utf16_cu_to_char(range.start),
            text.utf16_cu_to_char(range.end),
        );
        self.goto(start)?;
        self.delete_to(end)
    }

    /// Moves point to `p` like Emacs's `mouse-set-point`, dropping any pending prefix.
    fn click(&mut self, line: usize, col: usize) {
        let core = &mut *self.core;
        core.pending.clear();
        core.transient = None;
        core.prefix_arg = None;
        if let Some(id) = core.hint_timer.take() {
            core.timers.cancel(id);
        }
        core.hint_shown = false;
        core.message = None;
        // So the next typed char starts a new undo group.
        core.self_inserts = 0;
        core.window.set_active(false);
        core.last_command = Some("mouse-set-point");
        let buffer = core.current();
        let p = (buffer.char_idx(line, col)).unwrap_or(buffer.len_chars());
        if let Err(err) = self.goto(p) {
            tracing::debug!(%err, "click outside the buffer");
        }
    }

    /// Undoes the current buffer's last group, or redoes the last undone one, and restores point.
    fn undo(&mut self, redo: bool) {
        let core = &mut *self.core;
        let buffer = core
            .buffers
            .get_mut(core.window.buffer())
            .expect("current buffer is live");
        let point = if redo { buffer.redo() } else { buffer.undo() };
        match point {
            Some(p) => {
                if let Err(err) = core.window.set_point(&mut core.buffers, p) {
                    tracing::debug!(%err, "undo left point out of range");
                }
            }
            None => {
                let what = if redo { "redo" } else { "undo" };
                core.echo(format!("No further {what} information"));
            }
        }
    }

    /// Leaves `prefix` for the next command, and the keys that extend it for the next key.
    fn set_prefix(&mut self, prefix: Prefix) {
        let mut map = Keymap::default();
        for digit in 0..10 {
            map.bind(&digit.to_string(), "digit-argument")
                .expect("digits parse");
        }
        map.bind("C-u", "universal-argument").expect("C-u parses");
        if !matches!(prefix, Prefix::Number(_)) {
            map.bind("-", "negative-argument").expect("- parses");
        }
        self.core.prefix_arg = Some(prefix);
        self.core.transient = Some(map);
    }

    /// Sets the mark at point, pushing the old one; on a repeat, toggles the region instead.
    fn set_mark_command(&mut self, prefix: Option<Prefix>) -> Result<(), String> {
        if matches!(prefix, Some(Prefix::Universal(_))) {
            return self.pop_to_mark();
        }
        let core = &mut *self.core;
        if core.last_command == Some("set-mark-command")
            && core.window.mark(&core.buffers).is_some()
        {
            let active = !core.window.active();
            core.window.set_active(active);
            let what = if active { "activated" } else { "deactivated" };
            core.echo(format!("Mark {what}"));
            return Ok(());
        }
        self.push_mark(true)
    }

    /// Pushes the mark at point and says so.
    fn push_mark(&mut self, activate: bool) -> Result<(), String> {
        let core = &mut *self.core;
        let point = core.point();
        core.window
            .push_mark(&mut core.buffers, point, activate)
            .map_err(|err| err.to_string())?;
        core.echo("Mark set".into());
        Ok(())
    }

    /// Moves point to the mark and the mark to the ring's next one.
    fn pop_to_mark(&mut self) -> Result<(), String> {
        let mark = self.mark()?;
        self.goto(mark)?;
        self.core.window.pop_mark();
        // So a C-SPC right after sets a new mark rather than toggling, as in Emacs.
        self.core.this_command = Some("pop-to-mark-command");
        Ok(())
    }

    fn exchange_point_and_mark(&mut self) -> Result<(), String> {
        let mark = self.mark()?;
        let core = &mut *self.core;
        let point = core.point();
        core.window
            .set_mark(&mut core.buffers, point)
            .map_err(|err| err.to_string())?;
        core.window.set_active(true);
        self.goto(mark)
    }

    fn mark(&self) -> Result<usize, String> {
        let core = &*self.core;
        (core.window.mark(&core.buffers)).ok_or_else(|| "No mark set in this buffer".into())
    }

    /// Moves point to `p`, which must be in the buffer.
    fn goto(&mut self, p: usize) -> Result<(), String> {
        let core = &mut *self.core;
        core.window
            .set_point(&mut core.buffers, p)
            .map_err(|err| err.to_string())
    }

    /// Moves point to `target` clamped to the buffer, reporting an end it ran into.
    fn goto_clamped(&mut self, target: i64) -> Result<(), String> {
        let len = self.core.current().len_chars() as i64;
        self.goto(target.clamp(0, len) as usize)?;
        match target {
            t if t < 0 => Err("Beginning of buffer".into()),
            t if t > len => Err("End of buffer".into()),
            _ => Ok(()),
        }
    }

    fn forward_char(&mut self, n: i64) -> Result<(), String> {
        self.goto_clamped((self.core.point() as i64).saturating_add(n))
    }

    /// Moves `n` lines to the goal column, which a run of line motions keeps.
    fn next_line(&mut self, n: i64) -> Result<(), String> {
        let core = &mut *self.core;
        let (line, col) = core
            .current()
            .line_col(core.point())
            .expect("point is in the buffer");
        if !matches!(core.last_command, Some("next-line" | "previous-line")) {
            // shortcut: the goal is in chars, so tabs and wide chars shift it; use display columns once the display renders them.
            core.goal_column = col;
        }
        let buffer = core.current();
        let target = (line as i64).saturating_add(n);
        let last = buffer.len_lines() as i64 - 1;
        if !(0..=last).contains(&target) {
            return self.goto_clamped(if target < 0 { -1 } else { i64::MAX });
        }
        let p = buffer
            .char_idx(target as usize, core.goal_column)
            .expect("target line is in the buffer");
        self.goto(p)
    }

    /// Inserts `text` `n` times at point, moving point after it unless `stay`.
    fn insert_n(&mut self, text: &str, n: i64, stay: bool) -> Result<(), String> {
        let n = usize::try_from(n).map_err(|_| format!("Negative repetition argument {n}"))?;
        let point = self.core.point();
        // shortcut: a huge count allocates without limit, as in Emacs; cap it if that bites.
        let text = text.repeat(n);
        self.core
            .current_mut()
            .insert(point, &text)
            .map_err(|err| err.to_string())?;
        if stay {
            return Ok(());
        }
        self.goto(point + text.chars().count())
    }

    /// `delete-char` and `delete-backward-char`: without a prefix an active, non-empty region is
    /// deleted (not killed), as Emacs's `delete-active-region`; otherwise `delete_n`.
    fn delete_char(&mut self, no_prefix: bool, n: i64) -> Result<(), String> {
        let point = self.core.point();
        match self.core.window.mark(&self.core.buffers) {
            Some(mark) if no_prefix && self.core.window.active() && mark != point => {
                self.delete_to(mark)
            }
            _ => self.delete_n(n),
        }
    }

    /// Deletes `n` chars after point, or `-n` before it; one that would pass an end deletes nothing.
    fn delete_n(&mut self, n: i64) -> Result<(), String> {
        let target = (self.core.point() as i64).saturating_add(n);
        if target < 0 {
            return Err("Beginning of buffer".into());
        }
        if target > self.core.current().len_chars() as i64 {
            return Err("End of buffer".into());
        }
        self.delete_to(target as usize)
    }

    /// Deletes between point and `target`; point's marker moves to the range's start.
    fn delete_to(&mut self, target: usize) -> Result<(), String> {
        let point = self.core.point();
        self.core
            .current_mut()
            .delete(point.min(target)..point.max(target))
            .map_err(|err| err.to_string())
    }

    fn forward_word(&mut self, n: i64) -> Result<(), String> {
        self.goto(self.word_end(n))
    }

    /// Where `n` words on from point end, a word being a run of alphanumerics; backward when `n`
    /// is negative.
    fn word_end(&self, n: i64) -> usize {
        let buffer = self.core.current();
        let (text, len) = (buffer.text(), buffer.len_chars());
        let word = |i: usize| text.char(i).is_alphanumeric();
        let mut p = self.core.point();
        for _ in 0..n.unsigned_abs() {
            if n > 0 {
                while p < len && !word(p) {
                    p += 1;
                }
                while p < len && word(p) {
                    p += 1;
                }
            } else {
                while p > 0 && !word(p - 1) {
                    p -= 1;
                }
                while p > 0 && word(p - 1) {
                    p -= 1;
                }
            }
            if p == 0 || p == len {
                break;
            }
        }
        p
    }

    /// Moves `n - 1` lines, stopping at the first or last, then to that line's start or end.
    fn line_end(&mut self, n: i64, end: bool) -> Result<(), String> {
        let buffer = self.core.current();
        let (line, _) = buffer
            .line_col(self.core.point())
            .expect("point is in the buffer");
        let last = buffer.len_lines() as i64 - 1;
        let line = (line as i64)
            .saturating_add(n.saturating_sub(1))
            .clamp(0, last);
        let col = if end { usize::MAX } else { 0 };
        let p = buffer
            .char_idx(line as usize, col)
            .expect("line is in the buffer");
        self.goto(p)
    }

    /// Goes to the start, or the end, of the buffer; a number N goes N tenths in from that end,
    /// then to the next line's start.
    fn buffer_end(&mut self, prefix: Option<Prefix>, end: bool) -> Result<(), String> {
        if !self.core.window.active() && !matches!(prefix, Some(Prefix::Universal(_))) {
            self.push_mark(false)?;
        }
        let buffer = self.core.current();
        let len = buffer.len_chars() as i64;
        let Some(n) = prefix.filter(|p| !matches!(p, Prefix::Universal(_))) else {
            return self.goto(if end { len as usize } else { 0 });
        };
        let tenths = len.saturating_mul(n.value()) / 10;
        let p = (if end { len - tenths } else { tenths }).clamp(0, len) as usize;
        let (line, _) = buffer.line_col(p).expect("p is in the buffer");
        let next = buffer.char_idx(line + 1, 0).unwrap_or(len as usize);
        self.goto(next)
    }

    /// Scrolls `n` lines down the buffer, up when negative, keeping point in the window.
    fn scroll_lines(&mut self, n: i64) -> Result<(), String> {
        let core = &mut *self.core;
        let rows = core.window.rows();
        let top = core.window.scroll(&core.buffers, rows);
        let buffer = core.current();
        let last = buffer.len_lines() as i64 - 1;
        let new = (top as i64).saturating_add(n).clamp(0, last) as usize;
        if new == top && n != 0 {
            let end = if n > 0 { "End" } else { "Beginning" };
            return Err(format!("{end} of buffer"));
        }
        let (line, _) = buffer
            .line_col(core.point())
            .expect("point is in the buffer");
        let shown = line.clamp(new, new + rows - 1);
        let p = buffer
            .char_idx(shown, 0)
            .expect("shown line is in the buffer");
        core.window.set_top(new);
        if shown != line {
            self.goto(p)?;
        }
        Ok(())
    }

    /// Scrolls a screenful less two lines for no prefix, the other way for a lone minus, or
    /// the prefix's count of lines; `sign` is -1 to scroll up the buffer.
    fn scroll_command(&mut self, prefix: Option<Prefix>, sign: i64) -> Result<(), String> {
        let page = (self.core.window.rows() as i64 - 2).max(1);
        let n = match prefix {
            None => page,
            Some(Prefix::Minus) => -page,
            Some(p) => p.value(),
        };
        self.scroll_lines(n.saturating_mul(sign))
    }

    /// Puts point's line at a row: from the prefix, or middle, top, bottom on repeated calls.
    fn recenter(&mut self, prefix: Option<Prefix>) {
        let core = &mut *self.core;
        let rows = core.window.rows() as i64;
        // A prefixed call leaves the cycle where it was, as in Emacs.
        let row = match prefix {
            None => {
                let again = core.last_command == Some("recenter-top-bottom");
                core.recenter = if again { (core.recenter + 1) % 3 } else { 0 };
                [rows / 2, 0, rows - 1][core.recenter]
            }
            Some(Prefix::Universal(_)) => rows / 2,
            Some(p) => {
                let n = p.value();
                (if n < 0 { rows + n } else { n }).clamp(0, rows - 1)
            }
        };
        let (line, _) = core
            .current()
            .line_col(core.point())
            .expect("point is in the buffer");
        core.window.set_top(line.saturating_sub(row as usize));
    }

    /// Splits the selected window in two, the new one below or right of it.
    fn split_window(&mut self, side_by_side: bool) -> Result<(), String> {
        self.outside_minibuffer("Attempt to split minibuffer window")?;
        let core = &mut *self.core;
        let id = WindowId(core.next_window);
        core.next_window += 1;
        let new = core.window.split(&mut core.buffers);
        core.others.insert(id, new);
        core.tree.split(core.selected, id, side_by_side);
        Ok(())
    }

    /// Selects the window `n` on from the selected one in tree order, wrapping round.
    fn other_window(&mut self, n: i64) -> Result<(), String> {
        // shortcut: Emacs leaves the minibuffer active in another window; allow it once commands can come back to it.
        self.outside_minibuffer("Can't leave the minibuffer")?;
        let core = &mut *self.core;
        let leaves = core.tree.leaves();
        let at = leaves.iter().position(|&id| id == core.selected);
        let at = at.expect("the selected window is in the tree") as i64;
        let next = (at + n.rem_euclid(leaves.len() as i64)) as usize % leaves.len();
        core.select(leaves[next]);
        Ok(())
    }

    /// Deletes the selected window, selecting the sibling that takes its space.
    fn delete_window(&mut self) -> Result<(), String> {
        let core = &mut *self.core;
        let old = core.selected;
        let next = (core.tree.remove(old))
            .ok_or("Attempt to delete minibuffer or sole ordinary window")?;
        core.select(next);
        core.others
            .remove(&old)
            .expect("deleted window is live")
            .release(&mut core.buffers);
        Ok(())
    }

    /// Deletes every window but the selected one.
    fn delete_other_windows(&mut self) -> Result<(), String> {
        self.outside_minibuffer("Can't expand minibuffer to full frame")?;
        let core = &mut *self.core;
        for (_, window) in core.others.drain() {
            window.release(&mut core.buffers);
        }
        core.tree = Tree::Leaf(core.selected);
        Ok(())
    }

    /// Kills the buffer `name`, asking first when it is a modified file buffer.
    fn kill_buffer(&mut self, args: Args) -> Result<(), String> {
        let name = &args.strings[0];
        let id = (self.core.buffers.find(name)).ok_or_else(|| format!("No such buffer {name}"))?;
        let buffer = self.core.buffers.get(id).expect("found");
        if buffer.file.is_some() && buffer.modified() {
            match args.strings.get(1).map(String::as_str) {
                None => {
                    let command = self.core.commands["kill-buffer"];
                    return self.read_string(
                        command,
                        args,
                        "Buffer modified; kill anyway? ".into(),
                    );
                }
                Some("yes") => {}
                Some(_) => return Ok(()),
            }
        }
        self.core.kill_buffer(id);
        Ok(())
    }

    /// Visits `name`, taken from the current buffer's directory: switches to a buffer visiting
    /// it, or asks the app to read it.
    fn find_file(&mut self, name: &str) -> Result<(), String> {
        let path = self.core.directory().join(name);
        match self.core.buffers.visiting(&path) {
            Some(id) => self.core.switch_to(id),
            None => self.effects.push(Effect::Open(path)),
        }
        Ok(())
    }

    /// Offers to save each modified file buffer, then quits once those saves succeed. The
    /// strings are name/answer pairs for the buffers asked about, then the `exit anyway` answer.
    fn save_buffers_kill_terminal(&mut self, mut args: Args) -> Result<(), String> {
        let command = self.core.commands["save-buffers-kill-terminal"];
        let asked = |name: &str| args.strings.as_chunks::<2>().0.iter().any(|p| p[0] == name);
        let next = (self.core.buffers.recent())
            .find(|(_, b)| b.file.is_some() && b.modified() && !asked(&b.name))
            .map(|(_, b)| (b.name.clone(), b.file.clone().expect("a file buffer")));
        if args.strings.len().is_multiple_of(2)
            && let Some((name, path)) = next
        {
            args.strings.push(name);
            return self.read_string(command, args, format!("Save file {}? ", path.display()));
        }
        let yes: Vec<String> = (args.strings.as_chunks::<2>().0.iter())
            .filter(|p| p[1].eq_ignore_ascii_case("yes"))
            .map(|p| p[0].clone())
            .collect();
        let declined = yes.len() < args.strings.len() / 2;
        if declined && args.strings.len().is_multiple_of(2) {
            let prompt = "Modified buffers exist; exit anyway? ".into();
            return self.read_string(command, args, prompt);
        }
        let core = &mut *self.core;
        let saving: Vec<BufferId> = (yes.iter())
            .filter_map(|name| core.buffers.find(name))
            .filter(|&id| core.buffers.get(id).is_some_and(Buffer::modified))
            .collect();
        for &id in &saving {
            self.save(id)?;
        }
        // Declining to exit still saves the buffers answered `Yes`.
        if declined
            && args
                .strings
                .last()
                .is_none_or(|a| !a.eq_ignore_ascii_case("yes"))
        {
            return Ok(());
        }
        match saving.is_empty() {
            true => self.effects.push(Effect::Quit),
            false => self.core.quitting = saving,
        }
        Ok(())
    }

    fn save(&mut self, id: BufferId) -> Result<(), String> {
        let buffer = self.core.buffers.get_mut(id).expect("saved buffer is live");
        let Some(path) = buffer.file.clone() else {
            tracing::info!(name = buffer.name, "buffer visits no file");
            return Ok(());
        };
        if !buffer.modified() {
            return Err("(No changes need to be saved)".into());
        }
        let len = buffer.text().len_chars();
        if len > 0 && buffer.text().char(len - 1) != '\n' {
            buffer.insert(len, "\n").map_err(|err| err.to_string())?;
        }
        self.effects.push(Effect::Save {
            buffer: id,
            tick: buffer.tick(),
            path,
            bytes: buffer.encode(),
            hash: buffer.disk,
        });
        Ok(())
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
    /// The prompt being answered, while the minibuffer is open.
    minibuffer: Option<Minibuffer>,
    /// Each prompt's answers, newest first.
    history: HashMap<String, Vec<String>>,
    /// The fuzzy matcher for completion, made on first use.
    matcher: Option<Nucleo<(String, String)>>,
    /// The timer that polls the matcher again while it is still matching.
    repoll: Option<TimerId>,
    /// The column a run of line motions aims for.
    goal_column: usize,
    /// Where the last `recenter-top-bottom` put point's line: middle, top or bottom.
    recenter: usize,
    /// Kills, newest first.
    kill_ring: VecDeque<String>,
    /// The kill the next yank inserts.
    yank_index: usize,
    /// The clipboard's text for the key being handled, when its command asked for it.
    clipboard: Option<String>,
    /// The clipboard text the editor last wrote or took in, so it isn't pushed again.
    clipboard_seen: Option<String>,
    settings: Settings,
    commands: HashMap<&'static str, Command>,
    /// shortcut: one line for settings errors, cleared on the next key; the echo area (#36) replaces it.
    message: Option<String>,
    /// The buffers whose saves a quit waits for; empty when not quitting.
    quitting: Vec<BufferId>,
    /// Buffers with a recovery file, by the tick written to it.
    autosaved: HashMap<BufferId, u64>,
    /// The auto-save timers: restarted by each edit, and started by the first edit after a write.
    autosave_idle: Option<TimerId>,
    autosave_max: Option<TimerId>,
}

/// Whether `buffer` has edits a crash would lose: modified, and not read-only or hidden.
fn autosaves(buffer: &Buffer) -> bool {
    buffer.modified() && !buffer.read_only && !buffer.name.starts_with(' ')
}

impl Default for Core {
    fn default() -> Self {
        let mut buffers = Buffers::default();
        let scratch = buffers.create("*scratch*");
        let window = Window::new(&mut buffers, scratch);
        let builtins: [(&'static str, command::Handler); 56] = [
            ("self-insert-command", |ctx, args| {
                let c = match ctx.core.this_keys.last().map(|key| key.code) {
                    Some(KeyCode::Char(c)) => c,
                    _ => return Err("self-insert-command must be run by a character key".into()),
                };
                ctx.insert_n(c.encode_utf8(&mut [0; 4]), args.count(), false)
            }),
            ("newline", |ctx, args| {
                ctx.insert_n("\n", args.count(), false)
            }),
            // shortcut: no indentation yet, so the same as newline; the indentation issue indents here.
            ("electric-newline-and-maybe-indent", |ctx, args| {
                ctx.insert_n("\n", args.count(), false)
            }),
            ("open-line", |ctx, args| {
                ctx.insert_n("\n", args.count(), true)
            }),
            ("delete-char", |ctx, args| {
                ctx.delete_char(args.prefix.is_none(), args.count())
            }),
            ("delete-backward-char", |ctx, args| {
                ctx.delete_char(args.prefix.is_none(), args.count().saturating_neg())
            }),
            ("minibuffer-directory-delete-char", |ctx, args| {
                match ctx.core.component_start() {
                    Some(start) if args.prefix.is_none() && !ctx.core.window.active() => {
                        ctx.delete_to(start)
                    }
                    _ => ctx.delete_char(args.prefix.is_none(), args.count().saturating_neg()),
                }
            }),
            ("kill-word", |ctx, args| {
                ctx.kill_to(ctx.word_end(args.count()))
            }),
            ("backward-kill-word", |ctx, args| {
                ctx.kill_to(ctx.word_end(args.count().saturating_neg()))
            }),
            ("kill-line", |ctx, args| ctx.kill_line(args.prefix)),
            ("kill-region", |ctx, args| {
                ctx.kill_region(args.region, false)
            }),
            ("kill-ring-save", |ctx, args| {
                ctx.kill_region(args.region, true)
            }),
            ("yank", |ctx, args| ctx.yank(args.prefix)),
            ("yank-pop", |ctx, args| ctx.yank_pop(args.count())),
            ("forward-char", |ctx, args| ctx.forward_char(args.count())),
            ("backward-char", |ctx, args| {
                ctx.forward_char(args.count().saturating_neg())
            }),
            ("next-line", |ctx, args| ctx.next_line(args.count())),
            ("previous-line", |ctx, args| {
                ctx.next_line(args.count().saturating_neg())
            }),
            ("forward-word", |ctx, args| ctx.forward_word(args.count())),
            ("backward-word", |ctx, args| {
                ctx.forward_word(args.count().saturating_neg())
            }),
            ("move-beginning-of-line", |ctx, args| {
                ctx.line_end(args.count(), false)
            }),
            ("move-end-of-line", |ctx, args| {
                ctx.line_end(args.count(), true)
            }),
            ("beginning-of-buffer", |ctx, args| {
                ctx.buffer_end(args.prefix, false)
            }),
            ("end-of-buffer", |ctx, args| {
                ctx.buffer_end(args.prefix, true)
            }),
            ("scroll-up-command", |ctx, args| {
                ctx.scroll_command(args.prefix, 1)
            }),
            ("scroll-down-command", |ctx, args| {
                ctx.scroll_command(args.prefix, -1)
            }),
            ("recenter-top-bottom", |ctx, args| {
                ctx.recenter(args.prefix);
                Ok(())
            }),
            ("undo", |ctx, _| {
                ctx.undo(false);
                Ok(())
            }),
            ("undo-redo", |ctx, _| {
                ctx.undo(true);
                Ok(())
            }),
            ("save-buffer", |ctx, _| ctx.save(ctx.core.window.buffer())),
            ("save-buffers-kill-terminal", |ctx, args| {
                ctx.save_buffers_kill_terminal(args)
            }),
            ("find-file", |ctx, args| ctx.find_file(&args.strings[0])),
            ("reload-settings", |ctx, _| {
                ctx.effects.push(Effect::LoadSettings);
                Ok(())
            }),
            ("universal-argument", |ctx, args| {
                let prefix = match args.prefix {
                    None => Prefix::Universal(4),
                    Some(Prefix::Universal(n)) => Prefix::Universal(n.saturating_mul(4)),
                    Some(Prefix::Minus) => Prefix::Universal(-4),
                    // After digits C-u ends the number, so the next digit is not part of it.
                    number @ Some(Prefix::Number(_)) => {
                        ctx.core.prefix_arg = number;
                        return Ok(());
                    }
                };
                ctx.set_prefix(prefix);
                Ok(())
            }),
            ("digit-argument", |ctx, args| {
                let digit = match ctx.core.this_keys.last().map(|key| key.code) {
                    Some(KeyCode::Char(c)) => c.to_digit(10),
                    _ => None,
                }
                .ok_or("digit-argument must be run by a digit key")?;
                let digit = i64::from(digit);
                let prefix = match args.prefix {
                    Some(Prefix::Number(n)) => {
                        let digit = if n < 0 { -digit } else { digit };
                        Prefix::Number(n.saturating_mul(10).saturating_add(digit))
                    }
                    Some(Prefix::Minus) if digit == 0 => Prefix::Minus,
                    Some(Prefix::Minus) => Prefix::Number(-digit),
                    _ => Prefix::Number(digit),
                };
                ctx.set_prefix(prefix);
                Ok(())
            }),
            ("negative-argument", |ctx, args| {
                match args.prefix {
                    Some(Prefix::Number(n)) => ctx.set_prefix(Prefix::Number(n.saturating_neg())),
                    Some(Prefix::Minus) => {}
                    _ => ctx.set_prefix(Prefix::Minus),
                }
                Ok(())
            }),
            ("set-mark-command", |ctx, args| {
                ctx.set_mark_command(args.prefix)
            }),
            ("exchange-point-and-mark", |ctx, _| {
                ctx.exchange_point_and_mark()
            }),
            ("split-window-below", |ctx, _| ctx.split_window(false)),
            ("split-window-right", |ctx, _| ctx.split_window(true)),
            ("other-window", |ctx, args| ctx.other_window(args.count())),
            ("delete-window", |ctx, _| ctx.delete_window()),
            ("delete-other-windows", |ctx, _| ctx.delete_other_windows()),
            ("switch-to-buffer", |ctx, args| {
                ctx.core.switch_to_name(&args.strings[0]);
                Ok(())
            }),
            ("switch-to-buffer-other-window", |ctx, args| {
                if ctx.core.tree.leaves().len() == 1 {
                    ctx.split_window(false)?;
                }
                ctx.other_window(1)?;
                ctx.core.switch_to_name(&args.strings[0]);
                Ok(())
            }),
            ("kill-buffer", |ctx, args| ctx.kill_buffer(args)),
            ("exit-minibuffer", |ctx, _| ctx.exit_minibuffer(false)),
            ("minibuffer-exit-input", |ctx, _| ctx.exit_minibuffer(true)),
            ("abort-minibuffers", |ctx, _| {
                ctx.core.close_minibuffer().ok_or("Not in a minibuffer")?;
                ctx.core.echo("Quit".into());
                Ok(())
            }),
            ("minibuffer-next-completion", |ctx, args| {
                ctx.next_completion(args.count())
            }),
            ("minibuffer-previous-completion", |ctx, args| {
                ctx.next_completion(args.count().saturating_neg())
            }),
            ("minibuffer-complete", |ctx, _| {
                let text = ctx.core.selected_candidate().ok_or("No match")?;
                ctx.set_text(&text)
            }),
            ("previous-history-element", |ctx, args| {
                ctx.history_element(args.count())
            }),
            ("next-history-element", |ctx, args| {
                ctx.history_element(args.count().saturating_neg())
            }),
            ("execute-extended-command", |ctx, args| {
                let name = (ctx.core.commands.get_key_value(args.strings[0].as_str()))
                    .map(|(name, _)| *name)
                    .ok_or_else(|| format!("No command named {}", args.strings[0]))?;
                ctx.core.this_command = Some(name);
                let args = Args {
                    prefix: args.prefix,
                    ..Args::default()
                };
                ctx.call(name, args);
                Ok(())
            }),
            ("keyboard-quit", |ctx, _| {
                ctx.core.window.set_active(false);
                ctx.core.echo("Quit".into());
                Ok(())
            }),
        ];
        let mut commands: HashMap<_, _> = builtins
            .map(|(name, handler)| {
                let spec = ArgSpec::default();
                (
                    name,
                    Command {
                        name,
                        spec,
                        handler,
                    },
                )
            })
            .into();
        let kills = [
            "kill-word",
            "backward-kill-word",
            "kill-line",
            "kill-region",
        ];
        for name in kills.into_iter().chain(["kill-ring-save", "yank"]) {
            let spec: &mut ArgSpec = &mut commands.get_mut(name).expect("a builtin").spec;
            spec.clipboard = true;
            spec.region = matches!(name, "kill-region" | "kill-ring-save");
        }
        commands
            .get_mut("execute-extended-command")
            .expect("a builtin")
            .spec = ArgSpec {
            prompts: &["M-x "],
            completion: Some(|core, _| {
                let mut names: Vec<String> = core.commands.keys().map(|n| n.to_string()).collect();
                names.sort();
                names
            }),
            annotation: Some(|core, name| {
                let keys = core.where_is(name)?;
                Some(
                    keys.iter()
                        .map(Key::to_string)
                        .collect::<Vec<_>>()
                        .join(" "),
                )
            }),
            ..ArgSpec::default()
        };
        for name in ["switch-to-buffer", "switch-to-buffer-other-window"] {
            commands.get_mut(name).expect("a builtin").spec = ArgSpec {
                prompts: if name == "switch-to-buffer" {
                    &["Switch to buffer: "]
                } else {
                    &["Switch to buffer in other window: "]
                },
                completion: Some(|core, _| core.buffer_names(false)),
                ..ArgSpec::default()
            };
        }
        commands.get_mut("find-file").expect("a builtin").spec = ArgSpec {
            prompts: &["Find file: "],
            files: true,
            ..ArgSpec::default()
        };
        commands
            .get_mut("save-buffers-kill-terminal")
            .expect("a builtin")
            .spec = ArgSpec {
            // Both prompts are opened by the handler; an odd count is mid-pair, a `Save file` one.
            completion: Some(|_, args| match args.strings.len() % 2 {
                1 => vec!["Yes".into(), "No".into()],
                _ => vec!["No".into(), "Yes".into()],
            }),
            ..ArgSpec::default()
        };
        commands.get_mut("kill-buffer").expect("a builtin").spec = ArgSpec {
            prompts: &["Kill buffer: "],
            // The second prompt, opened by the handler, confirms killing a modified buffer.
            completion: Some(|core, args| match args.strings.is_empty() {
                true => core.buffer_names(true),
                false => vec!["no".into(), "yes".into()],
            }),
            ..ArgSpec::default()
        };
        let mut core = Core {
            buffers,
            window,
            selected: WindowId(0),
            others: HashMap::new(),
            tree: Tree::Leaf(WindowId(0)),
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
            minibuffer: None,
            history: HashMap::new(),
            matcher: None,
            repoll: None,
            goal_column: 0,
            recenter: 0,
            kill_ring: VecDeque::new(),
            yank_index: 0,
            clipboard: None,
            clipboard_seen: None,
            settings: Settings::default(),
            commands,
            message: None,
            quitting: Vec::new(),
            autosaved: HashMap::new(),
            autosave_idle: None,
            autosave_max: None,
        };
        let (keymaps, errors) = core.parse_keymaps(None).expect("bundled keymap parses");
        assert!(errors.is_empty(), "bundled keymap: {errors:?}");
        core.keymaps = keymaps;
        core
    }
}

impl Core {
    /// The bundled keymaps with the user's keymap file text, if any, bound over them.
    fn parse_keymaps(
        &self,
        user: Option<&str>,
    ) -> Result<(HashMap<String, Keymap>, Vec<String>), String> {
        keymap::parse(keymap::BUNDLED, user, |name| {
            self.commands.get_key_value(name).map(|(name, _)| *name)
        })
    }

    /// Shows `msg` in the echo area and logs it to `*Messages*`.
    fn echo(&mut self, msg: String) {
        self.log(&msg);
        self.message = Some(msg);
    }

    /// Appends `line` to `*Messages*`, making the buffer if there is none.
    fn log(&mut self, line: &str) {
        let id = self.buffers.find("*Messages*").unwrap_or_else(|| {
            let id = self.buffers.create("*Messages*");
            self.buffers.get_mut(id).expect("just created").read_only = true;
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

    /// The binding of `keys` in the first keymap that binds it: transient, the current
    /// buffer's minor modes, its major mode, then global.
    fn lookup(&self, keys: &[Key]) -> Option<&Binding> {
        self.layers().find_map(|map| map.lookup(keys))
    }

    /// Whether `key` would run a command that reads the clipboard, so the app sends
    /// `Msg::Clipboard` before it.
    pub fn wants_clipboard(&self, key: Key) -> bool {
        let keys = [&self.pending[..], &[key]].concat();
        matches!(self.lookup(&keys), Some(Binding::Command(name))
            if self.commands.get(name).is_some_and(|c| c.spec.clipboard))
    }

    /// Puts `text` on the kill ring as a new entry, dropping the oldest past the limit.
    fn kill_new(&mut self, text: String) {
        self.kill_ring.push_front(text);
        self.kill_ring.truncate(KILL_RING_MAX);
        self.yank_index = 0;
    }

    /// Moves the yank pointer `n` round the kill ring and returns that kill.
    fn current_kill(&mut self, n: i64) -> Result<String, String> {
        let len = self.kill_ring.len() as i64;
        if len == 0 {
            return Err("Kill ring is empty".into());
        }
        self.yank_index = (self.yank_index as i64 + n.rem_euclid(len)).rem_euclid(len) as usize;
        Ok(self.kill_ring[self.yank_index].clone())
    }

    /// The keymaps in lookup order.
    fn layers(&self) -> impl Iterator<Item = &Keymap> {
        let buffer = self.current();
        let modes = buffer
            .minor_modes
            .iter()
            .chain([&buffer.major_mode, &"global"]);
        self.transient
            .iter()
            .chain(modes.filter_map(|mode| self.keymaps.get(*mode)))
    }

    /// The shortest key sequence that runs `command` under keymap layering, the first in
    /// `kbd` text order among equally short ones.
    fn where_is(&self, command: &str) -> Option<Vec<Key>> {
        fn walk(map: &Keymap, keys: &mut Vec<Key>, command: &str, found: &mut Vec<Vec<Key>>) {
            for (key, binding) in map.bindings() {
                keys.push(*key);
                match binding {
                    Binding::Command(name) if *name == command => found.push(keys.clone()),
                    Binding::Command(_) => {}
                    Binding::Prefix(map) => walk(map, keys, command, found),
                }
                keys.pop();
            }
        }
        let mut found = Vec::new();
        for map in self.layers() {
            walk(map, &mut Vec::new(), command, &mut found);
        }
        // A key a higher layer binds to something else does not run it.
        found.retain(
            |keys| matches!(self.lookup(keys), Some(Binding::Command(name)) if *name == command),
        );
        let text = |keys: &Vec<Key>| {
            keys.iter()
                .map(Key::to_string)
                .collect::<Vec<_>>()
                .join(" ")
        };
        found
            .into_iter()
            .min_by_key(|keys| (keys.len(), text(keys)))
    }

    /// The hint for the pending prefix, once its delay has passed. Each next key shows the
    /// binding of the first layer that binds it, as `lookup` would find.
    pub fn hint(&self) -> Option<display::Hint> {
        if !self.hint_shown {
            return None;
        }
        let mut next = HashMap::new();
        for map in self.layers() {
            if let Some(Binding::Prefix(map)) = map.lookup(&self.pending) {
                for (key, binding) in map.bindings() {
                    next.entry(*key).or_insert(binding);
                }
            }
        }
        let mut entries: Vec<_> = next
            .into_iter()
            .map(|(key, binding)| {
                let what = match binding {
                    Binding::Command(name) => name.to_string(),
                    Binding::Prefix(_) => "+prefix".into(),
                };
                (key.to_string(), what)
            })
            .collect();
        entries.sort();
        let keys: Vec<String> = self.pending.iter().map(Key::to_string).collect();
        Some(display::Hint {
            keys: keys.join(" "),
            entries,
        })
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
        let mut effects = ctx.effects;
        self.autosave(now, input, &mut effects);
        self.poll_completion(now, &mut effects);
        effects
    }

    /// After input, (re)starts the auto-save timers if a buffer has unwritten edits; deletes the
    /// recovery files of buffers gone or saved, and all of them before a quit.
    fn autosave(&mut self, now: Instant, input: bool, effects: &mut Effects) {
        let quit = effects.iter().position(|e| matches!(e, Effect::Quit));
        let buffers = &self.buffers;
        let stale: Vec<BufferId> = (self.autosaved.keys().copied())
            .filter(|&id| quit.is_some() || !buffers.get(id).is_some_and(autosaves))
            .collect();
        let at = quit.unwrap_or(effects.len());
        for &id in &stale {
            self.autosaved.remove(&id);
        }
        effects.splice(at..at, stale.into_iter().map(Effect::DeleteAutoSave));
        let unwritten = (self.buffers.recent())
            .any(|(id, b)| autosaves(b) && self.autosaved.get(&id) != Some(&b.tick()));
        if input && unwritten {
            if let Some(id) = self.autosave_idle.take() {
                self.timers.cancel(id);
            }
            self.autosave_idle = Some(self.timers.start(now, AUTOSAVE_IDLE, None, Msg::AutoSave));
            if self.autosave_max.is_none() {
                self.autosave_max = Some(self.timers.start(now, AUTOSAVE_MAX, None, Msg::AutoSave));
            }
        }
    }

    /// Matches the completing minibuffer's text against its candidates, waiting up to 10 ms,
    /// and polls again on a timer while the matcher is still running. A file prompt matches the
    /// text after the last `/`, asking for a listing when the part before it changes.
    fn poll_completion(&mut self, now: Instant, effects: &mut Effects) {
        let text = self.text();
        let Some(completion) = (self.minibuffer.as_mut()).and_then(|mb| mb.completion.as_mut())
        else {
            return;
        };
        let matcher = self
            .matcher
            .as_mut()
            .expect("a completing minibuffer has a matcher");
        let (mut pattern, mut fresh) = (text.as_str(), false);
        if let Some(dir) = &mut completion.dir {
            let (head, tail) = text.split_at(text.rfind('/').map_or(0, |i| i + 1));
            pattern = tail;
            if head != dir {
                *dir = head.to_string();
                completion.listed = false;
                fresh = true;
                matcher.restart(true);
                if !head.is_empty() {
                    effects.push(Effect::ListDir(completion.base.join(head)));
                }
            }
        }
        if fresh || completion.pattern != pattern {
            // Text that extends the old pattern lets nucleo narrow the last matches.
            let append = !fresh && pattern.starts_with(&completion.pattern);
            (matcher.pattern).reparse(
                0,
                pattern,
                CaseMatching::Smart,
                Normalization::Smart,
                append,
            );
            completion.pattern = pattern.to_string();
            completion.select(0);
        }
        if let Some(id) = self.repoll.take() {
            self.timers.cancel(id);
        }
        if matcher.tick(10).running {
            self.repoll = Some(self.timers.start(now, REPOLL, None, Msg::Tick));
        }
    }

    /// How many candidates match, 0 when not completing.
    fn matches(&self) -> usize {
        match (&self.minibuffer, &self.matcher) {
            (
                Some(Minibuffer {
                    completion: Some(_),
                    ..
                }),
                Some(m),
            ) => m.snapshot().matched_item_count() as usize,
            _ => 0,
        }
    }

    /// The selected candidate, if the minibuffer is completing and anything matches.
    fn selected_candidate(&self) -> Option<String> {
        let completion = self.minibuffer.as_ref()?.completion.as_ref()?;
        let snapshot = self.matcher.as_ref()?.snapshot();
        let last = snapshot.matched_item_count().checked_sub(1)?;
        let item = snapshot.get_matched_item((completion.selected as u32).min(last))?;
        let name = &item.data.0;
        Some(match &completion.dir {
            Some(dir) => format!("{dir}{name}"),
            None => name.clone(),
        })
    }

    /// In a file prompt with point at the end of a text that ends in `/` or names a listed
    /// directory, the start of that last component: where `DEL` deletes back to.
    fn component_start(&self) -> Option<usize> {
        let completion = self.minibuffer.as_ref()?.completion.as_ref()?;
        let (dir, matcher) = (completion.dir.as_ref()?, self.matcher.as_ref()?);
        let text = self.text();
        if self.point() != text.chars().count() {
            return None;
        }
        let component = match text.strip_suffix('/') {
            Some(component) => component,
            None => {
                // The listing already loaded says whether the last component is a directory.
                let name = format!("{}/", text.strip_prefix(dir.as_str())?);
                let injector = matcher.injector();
                let listed = (0..injector.injected_items())
                    .any(|i| injector.get(i).is_some_and(|item| item.data.0 == name));
                listed.then_some(text.as_str())?
            }
        };
        let start = component.rfind('/')? + 1;
        Some(text[..start].chars().count())
    }

    /// The current buffer's directory: its file's, else the working directory.
    fn directory(&self) -> PathBuf {
        match self.current().file.as_deref().and_then(Path::parent) {
            Some(dir) => dir.to_path_buf(),
            None => std::env::current_dir().unwrap_or_default(),
        }
    }

    /// Visits `path` in a new current buffer, given the file's bytes and their hash, or `None`
    /// when there is no file. A binary or non-UTF-8 file opens empty and read-only.
    pub fn open(&mut self, path: &Path, file: Option<(Vec<u8>, u64)>) {
        self.visit(path, file, false)
    }

    /// `open`, but a buffer already visiting `path` is switched to, and with `refuse` a binary
    /// or non-UTF-8 file is not opened; either way the echo area says why.
    fn visit(&mut self, path: &Path, file: Option<(Vec<u8>, u64)>, refuse: bool) {
        // Two finds of one file can race; the later one switches to the first's buffer.
        if let Some(id) = self.buffers.visiting(path) {
            return self.switch_to(id);
        }
        let disk = file.as_ref().map(|(_, hash)| *hash);
        let (text, crlf, bom, refused) = match file.map(|(bytes, _)| buffer::decode(&bytes)) {
            None => (String::new(), false, false, None),
            Some(Ok((text, crlf, bom))) => (text, crlf, bom, None),
            Some(Err(reason)) => (String::new(), false, false, Some(reason)),
        };
        if let Some(reason) = refused {
            let opened = if refuse { "" } else { ", opened read-only" };
            self.echo(format!("{}: {reason}{opened}", path.display()));
            if refuse {
                return;
            }
        }
        let id = self.buffers.create_for_file(path, &text);
        let buffer = self.buffers.get_mut(id).expect("just created");
        (buffer.crlf, buffer.bom, buffer.disk) = (crlf, bom, disk);
        buffer.read_only = refused.is_some();
        self.switch_to(id);
    }

    /// Shows buffer `id` in the selected window and makes it the most recent.
    fn switch_to(&mut self, id: BufferId) {
        self.buffers.raise(id);
        if self.window.buffer() != id {
            self.window.show(&mut self.buffers, id);
        }
    }

    /// Switches to the buffer `name`, making an empty one if there is none.
    fn switch_to_name(&mut self, name: &str) {
        let id = (self.buffers.find(name)).unwrap_or_else(|| self.buffers.create(name));
        self.switch_to(id);
    }

    /// The names of the buffers not hidden by a leading space, most recently shown first, with
    /// the current buffer first or last.
    fn buffer_names(&self, current_first: bool) -> Vec<String> {
        let current = self.window.buffer();
        let listed = self
            .buffers
            .recent()
            .filter(|(_, b)| !b.name.starts_with(' '));
        let (this, others): (Vec<_>, Vec<_>) = listed.partition(|(id, _)| *id == current);
        let names = match current_first {
            true => [this, others].concat(),
            false => [others, this].concat(),
        };
        names.into_iter().map(|(_, b)| b.name.clone()).collect()
    }

    /// Kills buffer `id`; windows showing it show the most recent listed buffer left, or a new
    /// `*scratch*`.
    fn kill_buffer(&mut self, id: BufferId) {
        let next = (self.buffers.recent())
            .find(|&(b, buffer)| b != id && !buffer.name.starts_with(' '))
            .map(|(b, _)| b);
        // Killed first, so a new `*scratch*` gets its name.
        _ = self.buffers.kill(id);
        let next = next.unwrap_or_else(|| self.buffers.create("*scratch*"));
        for window in std::iter::once(&mut self.window).chain(self.others.values_mut()) {
            if window.buffer() == id {
                window.show(&mut self.buffers, next);
            }
        }
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

    /// The selected window's display model for a window `height` px high, mode line included.
    pub fn display(&mut self, height: f32) -> display::WindowDisplay {
        window_display(&mut self.window, &self.buffers, &self.settings, height)
    }

    /// Every window's display model, laid out in the size last reported by `Msg::Resize`.
    pub fn frame(&mut self) -> display::FrameDisplay {
        let (width, height, cell_width) = self.frame_size;
        let cell = (cell_width, self.settings.line_height());
        let frame = display::Rect {
            x: 0.,
            y: 0.,
            width,
            height,
        };
        let windows = (self.tree.layout(frame, cell).into_iter())
            .map(|(id, rect)| {
                let selected = id == self.selected;
                let window = match selected {
                    true => &mut self.window,
                    false => self.others.get_mut(&id).expect("tree windows are live"),
                };
                let display = window_display(window, &self.buffers, &self.settings, rect.height);
                display::FrameWindow {
                    id,
                    rect,
                    selected,
                    display,
                }
            })
            .collect();
        display::FrameDisplay {
            windows,
            message: self.message.clone(),
            minibuffer: self.minibuffer(),
        }
    }

    /// The open minibuffer's prompt and the line holding its point.
    pub fn minibuffer(&mut self) -> Option<display::Minibuffer> {
        let mb = self.minibuffer.as_ref()?;
        let prompt = mb.prompt.to_string();
        let (mut candidates, mut annotations, mut selected) = (Vec::new(), Vec::new(), None);
        if let (Some(completion), Some(matcher)) = (&mb.completion, &mut self.matcher) {
            matcher.tick(0);
            let snapshot = matcher.snapshot();
            let count = snapshot.matched_item_count() as usize;
            let rows = completion.top.min(count)..(completion.top + COMPLETION_ROWS).min(count);
            let items = snapshot.matched_items(rows.start as u32..rows.end as u32);
            (candidates, annotations) = items.map(|item| item.data.clone()).unzip();
            selected = (count > 0).then(|| completion.selected.min(count - 1) - rows.start);
        }
        // The minibuffer is selected while open; one row, as the height leaves out a mode line.
        let height = 2. * self.settings.line_height();
        let display = window_display(&mut self.window, &self.buffers, &self.settings, height);
        Some(display::Minibuffer {
            prompt,
            display,
            candidates,
            annotations,
            selected,
        })
    }

    /// Closes the minibuffer, selecting the window it was opened from again.
    fn close_minibuffer(&mut self) -> Option<Minibuffer> {
        let mb = self.minibuffer.take()?;
        self.select(mb.from);
        let window = (self.others.remove(&MINIBUFFER)).expect("the minibuffer window is live");
        _ = self.buffers.kill(window.buffer());
        Some(mb)
    }

    /// Makes window `id` the selected one.
    fn select(&mut self, id: WindowId) {
        if id == self.selected {
            return;
        }
        let window = self.others.remove(&id).expect("selected window is live");
        let old = std::mem::replace(&mut self.window, window);
        self.others.insert(self.selected, old);
        self.selected = id;
    }

    /// The window's first line once point's line is among the `rows` it shows.
    pub fn scroll(&mut self, rows: usize) -> usize {
        self.window.scroll(&self.buffers, rows)
    }
}

/// `window`'s display model for a window `height` px high, mode line included.
fn window_display(
    window: &mut Window,
    buffers: &Buffers,
    settings: &Settings,
    height: f32,
) -> display::WindowDisplay {
    let rows = ((height / settings.line_height()) as usize)
        .saturating_sub(1)
        .max(1);
    // shortcut: scrolls in buffer lines, so folds above point scroll it further than needed and a top line inside a fold repeats its placeholder; scroll in display lines when folds get common.
    let top = window.scroll(buffers, rows);
    let buffer = buffers.get(window.buffer()).expect("window buffer is live");
    let point = window.point(buffers);
    let mark = window.mark(buffers).filter(|_| window.active());
    display::build(buffer, top, rows, point, mark, settings)
}
