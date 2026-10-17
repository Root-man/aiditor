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

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use ropey::Rope;

use buffer::{Buffer, BufferId, Buffers};
use command::{ArgSpec, Args, Command, Prefix};
use key::{Key, KeyCode};
use keymap::{Binding, Keymap};
use settings::Settings;
use timer::{TimerId, Timers};
use window::Window;

#[derive(Clone, Debug, PartialEq)]
pub enum Msg {
    Key(Key),
    /// Wakes the core when a timer is due; due timers fire on every message.
    Tick,
    /// The app wrote the buffer's text to its file.
    Saved(BufferId),
    /// The user's settings file: its text, `None` if it doesn't exist, or why it couldn't be read.
    SettingsLoaded(Result<Option<String>, String>),
    /// The user's keymap file, the same way.
    KeymapLoaded(Result<Option<String>, String>),
    /// The prefix hint's delay passed with the prefix still pending.
    Hint,
}

/// How long a prefix waits before the hint shows, Emacs's `echo-keystrokes`.
const HINT_DELAY: Duration = Duration::from_secs(1);

/// Work the app carries out for the core.
pub enum Effect {
    /// Write `text` to `path`, then send `Msg::Saved(buffer)` if that worked.
    Save {
        buffer: BufferId,
        path: PathBuf,
        text: Rope,
    },
    /// Read the user's settings and keymap files and send `Msg::SettingsLoaded` and
    /// `Msg::KeymapLoaded`.
    LoadSettings,
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
            self.core.message = Some(err);
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
        if let Some(prompt) = command.spec.prompts.get(args.strings.len()) {
            return Err(format!("{name}: no answer for {prompt:?}"));
        }
        (command.handler)(self, args)
    }

    fn dispatch(&mut self, msg: Msg) {
        match msg {
            Msg::Key(key) => self.key(key),
            Msg::Saved(id) => {
                if let Some(buffer) = self.core.buffers.get_mut(id) {
                    buffer.set_saved();
                }
            }
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
            core.message = Some("Quit".into());
            core.last_command = Some("keyboard-quit");
            return;
        }
        core.pending.push(key);
        let command = match core.lookup() {
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
        core.message = Some(format!("{} is undefined", keys.join(" ")));
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
                core.message = Some(format!("No further {what} information"));
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
            core.message = Some(format!("Mark {what}"));
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
        core.message = Some("Mark set".into());
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

    fn save(&mut self) {
        let id = self.core.window.buffer();
        let buffer = self.core.current();
        match &buffer.file {
            Some(path) => self.effects.push(Effect::Save {
                buffer: id,
                path: path.clone(),
                text: buffer.text().clone(),
            }),
            None => tracing::info!(name = buffer.name, "buffer visits no file"),
        }
    }
}

pub struct Core {
    buffers: Buffers,
    /// The frame's one window, showing the current buffer, until splits land.
    window: Window,
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
    /// The column a run of line motions aims for.
    goal_column: usize,
    /// Where the last `recenter-top-bottom` put point's line: middle, top or bottom.
    recenter: usize,
    settings: Settings,
    commands: HashMap<&'static str, Command>,
    /// shortcut: one line for settings errors, cleared on the next key; the echo area (#36) replaces it.
    message: Option<String>,
}

impl Default for Core {
    fn default() -> Self {
        let mut buffers = Buffers::default();
        let scratch = buffers.create("*scratch*");
        let window = Window::new(&mut buffers, scratch);
        let builtins: [(&'static str, command::Handler); 31] = [
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
            ("delete-char", |ctx, args| ctx.delete_n(args.count())),
            ("delete-backward-char", |ctx, args| {
                ctx.delete_n(args.count().saturating_neg())
            }),
            // shortcut: the killed text is dropped, not saved; the kill-ring issue saves it.
            ("kill-word", |ctx, args| {
                ctx.delete_to(ctx.word_end(args.count()))
            }),
            ("backward-kill-word", |ctx, args| {
                ctx.delete_to(ctx.word_end(args.count().saturating_neg()))
            }),
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
            ("save-buffer", |ctx, _| {
                ctx.save();
                Ok(())
            }),
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
            ("keyboard-quit", |ctx, _| {
                ctx.core.window.set_active(false);
                ctx.core.message = Some("Quit".into());
                Ok(())
            }),
        ];
        let commands = builtins
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
        let mut core = Core {
            buffers,
            window,
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
            goal_column: 0,
            recenter: 0,
            settings: Settings::default(),
            commands,
            message: None,
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

    /// Shows `msg` on the message line, after what a load already put there this key.
    fn report(&mut self, msg: String) {
        self.message = Some(match self.message.take() {
            Some(old) => format!("{old}; {msg}"),
            None => msg,
        });
    }

    /// The pending sequence's binding in the first keymap that binds it: transient, the current
    /// buffer's minor modes, its major mode, then global.
    fn lookup(&self) -> Option<&Binding> {
        self.layers().find_map(|map| map.lookup(&self.pending))
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
        ctx.effects
    }

    /// Visits `path` in a new current buffer holding `text`, already read from it.
    pub fn open(&mut self, path: &Path, text: &str) {
        let id = self.buffers.create_for_file(path, text);
        self.window.show(&mut self.buffers, id);
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

    /// The window's display model for a window `height` px high, mode line included.
    pub fn display(&mut self, height: f32) -> display::WindowDisplay {
        let rows = ((height / self.settings.line_height()) as usize)
            .saturating_sub(1)
            .max(1);
        // shortcut: scrolls in buffer lines, so folds above point scroll it further than needed and a top line inside a fold repeats its placeholder; scroll in display lines when folds get common.
        let top = self.window.scroll(&self.buffers, rows);
        let buffer = self.current();
        let point = self.point();
        let mark = (self.window.mark(&self.buffers)).filter(|_| self.window.active());
        display::build(buffer, top, rows, point, mark, &self.settings)
    }

    /// The window's first line once point's line is among the `rows` it shows.
    pub fn scroll(&mut self, rows: usize) -> usize {
        self.window.scroll(&self.buffers, rows)
    }
}
