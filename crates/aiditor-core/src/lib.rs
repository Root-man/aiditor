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
}

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
        if let Err(err) = self.try_call(name, args) {
            self.core.message = Some(err);
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
            Msg::Tick => {}
        }
    }

    /// Adds `key` to the pending sequence and runs what the keymaps bind it to; an unbound
    /// plain key falls back to the hardcoded editing keys.
    fn key(&mut self, key: Key) {
        let core = &mut *self.core;
        if let Some(keys) = &mut core.recording {
            keys.push(key);
        }
        core.message = None;
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
            Some(Binding::Prefix(_)) => return,
            Some(Binding::Command(name)) => Some(*name),
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
        let undefined = |core: &mut Core| {
            let keys: Vec<String> = keys.iter().map(Key::to_string).collect();
            core.message = Some(format!("{} is undefined", keys.join(" ")));
        };
        if keys.len() > 1 || key.ctrl || key.meta {
            return undefined(core);
        }
        let buffer = core
            .buffers
            .get_mut(core.window.buffer())
            .expect("current buffer is live");
        let moved = match key.code {
            KeyCode::Char(c) => buffer
                .insert(point, c.encode_utf8(&mut [0; 4]))
                .map(|()| point + 1),
            KeyCode::Return => buffer.insert(point, "\n").map(|()| point + 1),
            // At 0 this deletes the empty range, a no-op.
            KeyCode::Backspace => buffer
                .delete(point.saturating_sub(1)..point)
                .map(|()| point.saturating_sub(1)),
            KeyCode::Left => Ok(point.saturating_sub(1)),
            KeyCode::Right => Ok((point + 1).min(buffer.len_chars())),
            // shortcut: no goal column, so a short line in between loses the column; the motion commands add one.
            KeyCode::Up => buffer
                .line_col(point)
                .and_then(|(line, col)| buffer.char_idx(line.saturating_sub(1), col)),
            KeyCode::Down => buffer
                .line_col(point)
                .map(|(line, col)| buffer.char_idx(line + 1, col).unwrap_or(point)),
            KeyCode::Tab | KeyCode::Escape => return undefined(core),
        };
        match moved.and_then(|p| core.window.set_point(&mut core.buffers, p)) {
            Ok(()) => {}
            Err(err) => tracing::debug!(%err, ?key, "edit failed"),
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

/// shortcut: unbound plain keys edit through hardcoded code; the editing commands bind them.
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
        let builtins: [(&'static str, command::Handler); 8] = [
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
            ("keyboard-quit", |ctx, _| {
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
            this_command: None,
            last_command: None,
            prefix_arg: None,
            this_keys: Vec::new(),
            recording: None,
            self_inserts: 0,
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
        let buffer = self.current();
        let modes = buffer
            .minor_modes
            .iter()
            .chain([&buffer.major_mode, &"global"]);
        self.transient
            .iter()
            .chain(modes.filter_map(|mode| self.keymaps.get(*mode)))
            .find_map(|map| map.lookup(&self.pending))
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
        let (point, mark) = (self.point(), self.window.mark(&self.buffers));
        display::build(buffer, top, rows, point, mark, &self.settings)
    }

    /// The window's first line once point's line is among the `rows` it shows.
    pub fn scroll(&mut self, rows: usize) -> usize {
        self.window.scroll(&self.buffers, rows)
    }
}
