//! Editor state. A plain state machine with no GPUI or Tokio dependency.

pub mod buffer;
#[cfg(test)]
mod harness;
pub mod key;
pub mod timer;

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use ropey::Rope;

use buffer::{Buffer, BufferId, Buffers};
use key::{Key, KeyCode};
use timer::{TimerId, Timers};

#[derive(Clone, Debug, PartialEq)]
pub enum Msg {
    Key(Key),
    /// Wakes the core when a timer is due; due timers fire on every message.
    Tick,
    /// The app wrote the buffer's text to its file.
    Saved(BufferId),
}

/// Work the app carries out for the core.
pub enum Effect {
    /// Write `text` to `path`, then send `Msg::Saved(buffer)` if that worked.
    Save {
        buffer: BufferId,
        path: PathBuf,
        text: Rope,
    },
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

    fn dispatch(&mut self, msg: Msg) {
        match msg {
            Msg::Key(key) => self.key(key),
            Msg::Saved(id) => {
                if let Some(buffer) = self.core.buffers.get_mut(id) {
                    buffer.set_saved();
                }
            }
            Msg::Tick => {}
        }
    }

    fn key(&mut self, key: Key) {
        let core = &mut *self.core;
        let ctrl = |c| Key {
            ctrl: true,
            ..Key::plain(KeyCode::Char(c))
        };
        if std::mem::take(&mut core.prefix_c_x) {
            return if key == ctrl('s') {
                self.save()
            } else {
                tracing::debug!(?key, "C-x key is undefined")
            };
        }
        if key == ctrl('x') {
            core.prefix_c_x = true;
            return;
        }
        let buffer = core
            .buffers
            .get_mut(core.current)
            .expect("current buffer is live");
        let point = core.point;
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
            // shortcut: no goal column, so a short line in between loses the column; add one with the window model.
            KeyCode::Up => buffer
                .line_col(point)
                .and_then(|(line, col)| buffer.char_idx(line.saturating_sub(1), col)),
            KeyCode::Down => buffer
                .line_col(point)
                .map(|(line, col)| buffer.char_idx(line + 1, col).unwrap_or(point)),
            _ => return tracing::debug!(?key, "key is undefined"),
        };
        match moved {
            Ok(p) => core.point = p,
            Err(err) => tracing::debug!(%err, ?key, "edit failed"),
        }
    }

    fn save(&mut self) {
        let id = self.core.current;
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

/// shortcut: one current buffer and hardcoded editing keys; the window model and command loop replace it.
pub struct Core {
    buffers: Buffers,
    current: BufferId,
    point: usize,
    timers: Timers,
    /// shortcut: `C-x` was the last key; the key trie replaces it.
    prefix_c_x: bool,
}

impl Default for Core {
    fn default() -> Self {
        let mut buffers = Buffers::default();
        let current = buffers.create("*scratch*");
        Core {
            buffers,
            current,
            point: 0,
            timers: Timers::default(),
            prefix_c_x: false,
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
        self.current = self.buffers.create_for_file(path, text);
        self.point = 0;
    }

    /// When the app must wake the core with `Msg::Tick`, if any timer is live.
    pub fn next_deadline(&self) -> Option<Instant> {
        self.timers.next_deadline()
    }

    pub fn current(&self) -> &Buffer {
        self.buffers
            .get(self.current)
            .expect("current buffer is live")
    }

    pub fn text(&self) -> String {
        self.current().text().to_string()
    }

    /// Point as a char offset into the buffer.
    pub fn point(&self) -> usize {
        self.point
    }
}
