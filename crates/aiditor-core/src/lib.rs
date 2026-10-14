//! Editor state. A plain state machine with no GPUI or Tokio dependency.

#[cfg(test)]
mod harness;
pub mod key;

use key::{Key, KeyCode};
use ropey::Rope;

pub enum Msg {
    Key(Key),
}

/// shortcut: one scratch buffer and hardcoded self-insert; the buffer registry and command loop replace it.
#[derive(Default)]
pub struct Core {
    text: Rope,
    point: usize,
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
        match c {
            Some(c) => {
                self.text.insert_char(self.point, c);
                self.point += 1;
            }
            None => tracing::debug!(?key, "key is undefined"),
        }
    }

    pub fn text(&self) -> String {
        self.text.to_string()
    }

    /// Point as a char offset into the buffer.
    pub fn point(&self) -> usize {
        self.point
    }
}
