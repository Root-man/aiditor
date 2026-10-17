//! Editor state. A plain state machine with no GPUI or Tokio dependency.

pub mod buffer;
#[cfg(test)]
mod harness;
pub mod key;

use buffer::{Buffer, BufferId, Buffers};
use key::{Key, KeyCode};

pub enum Msg {
    Key(Key),
}

/// shortcut: one current buffer and hardcoded self-insert; the window model and command loop replace it.
pub struct Core {
    buffers: Buffers,
    current: BufferId,
    point: usize,
}

impl Default for Core {
    fn default() -> Self {
        let mut buffers = Buffers::default();
        let current = buffers.create("*scratch*");
        Core {
            buffers,
            current,
            point: 0,
        }
    }
}

impl Core {
    pub fn handle(&mut self, msg: Msg) {
        match msg {
            Msg::Key(key) => self.key(key),
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
        let point = self.point;
        let buffer = self
            .buffers
            .get_mut(self.current)
            .expect("current buffer is live");
        match buffer.insert(point, c.encode_utf8(&mut [0; 4])) {
            Ok(()) => self.point += 1,
            Err(err) => tracing::debug!(%err, "self-insert failed"),
        }
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
