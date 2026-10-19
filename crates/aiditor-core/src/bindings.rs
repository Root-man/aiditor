//! What key sequences are bound to under keymap layering.

use std::collections::HashMap;
use std::time::Duration;

use crate::key::Key;
use crate::keymap::{self, Binding, Keymap};
use crate::{Core, display};

/// How long a prefix waits before the hint shows, Emacs's `echo-keystrokes`.
pub(crate) const HINT_DELAY: Duration = Duration::from_secs(1);

impl Core {
    /// The bundled keymaps with the user's keymap file text, if any, bound over them.
    pub(crate) fn parse_keymaps(
        &self,
        user: Option<&str>,
    ) -> Result<(HashMap<String, Keymap>, Vec<String>), String> {
        keymap::parse(keymap::BUNDLED, user, |name| {
            self.commands.get_key_value(name).map(|(name, _)| *name)
        })
    }

    /// The binding of `keys` in the first keymap that binds it: transient, the current
    /// buffer's minor modes, its major mode, then global.
    pub(crate) fn lookup(&self, keys: &[Key]) -> Option<&Binding> {
        self.layers().find_map(|map| map.lookup(keys))
    }

    /// Whether `key` would run a command that reads the clipboard, so the app sends
    /// `Msg::Clipboard` before it.
    pub fn wants_clipboard(&self, key: Key) -> bool {
        let keys = [&self.pending[..], &[key]].concat();
        matches!(self.lookup(&keys), Some(Binding::Command(name))
            if self.commands.get(name).is_some_and(|c| c.spec.clipboard))
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
    pub(crate) fn where_is(&self, command: &str) -> Option<Vec<Key>> {
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
}
