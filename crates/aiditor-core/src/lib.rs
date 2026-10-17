//! Editor state. A plain state machine with no GPUI or Tokio dependency.

pub mod buffer;
#[cfg(test)]
mod harness;
pub mod key;
pub mod timer;

use std::time::{Duration, Instant};

use buffer::{Buffer, BufferId, Buffers};
use key::{Key, KeyCode};
use timer::{TimerId, Timers};

#[derive(Clone, Debug, PartialEq)]
pub enum Msg {
    Key(Key),
    /// Wakes the core when a timer is due; due timers fire on every message.
    Tick,
}

/// Work the app carries out for the core. Variants come with the first service request.
pub enum Effect {}

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
            Msg::Tick => {}
        }
    }

    fn key(&mut self, key: Key) {
        let c = match key.code {
            _ if key.ctrl || key.meta => None,
            KeyCode::Char(c) => Some(c),
            KeyCode::Return => Some('\n'),
            _ => None,
        };
        let Some(c) = c else {
            return tracing::debug!(?key, "key is undefined");
        };
        let core = &mut *self.core;
        let buffer = core
            .buffers
            .get_mut(core.current)
            .expect("current buffer is live");
        match buffer.insert(core.point, c.encode_utf8(&mut [0; 4])) {
            Ok(()) => core.point += 1,
            Err(err) => tracing::debug!(%err, "self-insert failed"),
        }
    }
}

/// shortcut: one current buffer and hardcoded self-insert; the window model and command loop replace it.
pub struct Core {
    buffers: Buffers,
    current: BufferId,
    point: usize,
    timers: Timers,
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
