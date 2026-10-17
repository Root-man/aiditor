//! Editor state. A plain state machine with no GPUI or Tokio dependency.

pub mod buffer;
pub mod command;
pub mod display;
#[cfg(test)]
mod harness;
pub mod key;
pub mod settings;
pub mod timer;
pub mod window;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use ropey::Rope;

use buffer::{Buffer, BufferId, Buffers};
use command::{ArgSpec, Args, Command};
use key::{Key, KeyCode};
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
}

/// Work the app carries out for the core.
pub enum Effect {
    /// Write `text` to `path`, then send `Msg::Saved(buffer)` if that worked.
    Save {
        buffer: BufferId,
        path: PathBuf,
        text: Rope,
    },
    /// Read the user's settings file and send `Msg::SettingsLoaded`.
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
                    Err(err) => self.core.message = Some(format!("settings: {err}")),
                }
            }
            Msg::Tick => {}
        }
    }

    fn key(&mut self, key: Key) {
        let core = &mut *self.core;
        core.message = None;
        let ctrl = |c| Key {
            ctrl: true,
            ..Key::plain(KeyCode::Char(c))
        };
        let point = core.window.point(&core.buffers);
        let buffer = core
            .buffers
            .get_mut(core.window.buffer())
            .expect("current buffer is live");
        // Typed chars share an undo group, up to 20 like Emacs; any other key ends the run.
        let self_insert =
            matches!(key.code, KeyCode::Char(_)) && !key.ctrl && !key.meta && !core.prefix_c_x;
        if !(self_insert && (1..20).contains(&core.self_inserts)) {
            buffer.undo_boundary(point);
            core.self_inserts = 0;
        }
        core.self_inserts += usize::from(self_insert);
        // Linux reports C-? and C-_ with shift held, macOS without.
        let unshifted = Key {
            shift: false,
            ..key
        };
        let prefixed = std::mem::take(&mut core.prefix_c_x);
        if unshifted == ctrl('/')
            || unshifted == ctrl('_')
            || (prefixed && key == Key::plain(KeyCode::Char('u')))
        {
            return self.call("undo", Args::default());
        }
        if unshifted == ctrl('?') {
            return self.call("undo-redo", Args::default());
        }
        if prefixed {
            return if key == ctrl('s') {
                self.call("save-buffer", Args::default())
            } else if key == ctrl('r') {
                self.call("reload-settings", Args::default())
            } else {
                tracing::debug!(?key, "C-x key is undefined")
            };
        }
        if key == ctrl('x') {
            core.prefix_c_x = true;
            return;
        }
        let moved = match key.code {
            _ if key.ctrl || key.meta => return tracing::debug!(?key, "key is undefined"),
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
            _ => return tracing::debug!(?key, "key is undefined"),
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

/// shortcut: hardcoded editing keys; the command loop replaces them.
pub struct Core {
    buffers: Buffers,
    /// The frame's one window, showing the current buffer, until splits land.
    window: Window,
    timers: Timers,
    /// shortcut: `C-x` was the last key; the key trie replaces it.
    prefix_c_x: bool,
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
        let builtins: [(&'static str, command::Handler); 4] = [
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
        Core {
            buffers,
            window,
            timers: Timers::default(),
            prefix_c_x: false,
            self_inserts: 0,
            settings: Settings::default(),
            commands,
            message: None,
        }
    }
}

impl Core {
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
