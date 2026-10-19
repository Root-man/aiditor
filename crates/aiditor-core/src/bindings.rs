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

#[cfg(test)]
mod tests {
    use crate::Msg;
    use crate::command::ArgSpec;
    use crate::command::Command;
    use crate::harness::Harness;
    use crate::harness::MS;
    use crate::harness::bind_mode;
    use crate::harness::buffer;
    use crate::settings::Settings;

    #[test]
    fn c_x_then_another_key_does_nothing() {
        let mut h = Harness::new(&[("a.txt", "")]);
        h.open("a.txt");
        h.keys("C-x a C-x C-x b");
        assert_eq!(h.text(), "b");
        assert_eq!(h.disk("a.txt"), "");
    }

    #[test]
    fn c_x_c_r_reloads_settings_and_keeps_them_on_error() {
        let mut h = Harness::new(&[("settings.toml", "font_size = 18")]);
        assert_eq!(h.core.settings().font_size, 14.0);
        h.keys("C-x C-r");
        assert_eq!(h.core.settings().font_size, 18.0);
        assert_eq!(h.core.message(), None);
        std::fs::write(h.path("settings.toml"), "font_size = \"big\"").unwrap();
        h.keys("C-x C-r");
        assert_eq!(h.core.settings().font_size, 18.0);
        assert!(h.core.message().unwrap().starts_with("settings: "));
        h.keys("a");
        assert_eq!(h.core.message(), None);
        assert_eq!(h.text(), "a");
    }

    #[test]
    fn c_x_c_r_reloads_keymaps_and_reverts_removed_bindings() {
        let mut h = Harness::new(&[("keymap.toml", "[global]\n\"C-x u\" = \"undo-redo\"")]);
        h.keys("C-x C-r ab C-/");
        assert_eq!(h.text(), "");
        h.keys("C-x u");
        assert_eq!(h.text(), "ab");
        std::fs::remove_file(h.path("keymap.toml")).unwrap();
        h.keys("C-x C-r C-x u");
        assert_eq!(h.text(), "");
        assert_eq!(h.core.message(), None);
    }

    #[test]
    fn changed_binding_takes_effect_on_reload() {
        let mut h = Harness::new(&[("keymap.toml", "[global]\n\"C-c a\" = \"one\"")]);
        echo_commands(&mut h, "C-c z");
        h.keys("C-x C-r C-c a");
        assert_eq!(h.core.message(), Some("one"));
        std::fs::write(h.path("keymap.toml"), "[global]\n\"C-c a\" = \"two\"").unwrap();
        h.keys("C-x C-r C-c a");
        assert_eq!(h.core.message(), Some("two"));
    }

    #[test]
    fn bad_keymap_entries_are_reported_and_the_rest_loads() {
        let user = "[global]\n\"C-c a\" = \"one\"\n\"C-c b\" = \"no-such-command\"\n\"C-xy\" = \"one\"\n[test-mode]\n\"C-c m\" = \"two\"";
        let mut h = Harness::new(&[("keymap.toml", user)]);
        echo_commands(&mut h, "C-c z");
        h.keys("C-x C-r");
        let msg = h.core.message().unwrap().to_owned();
        assert!(msg.starts_with("keymap: "), "{msg}");
        for needle in ["C-c b", "no-such-command", "C-xy"] {
            assert!(msg.contains(needle), "{needle}: {msg}");
        }
        h.keys("C-c a");
        assert_eq!(h.core.message(), Some("one"));
        buffer(&mut h).minor_modes.push("test-mode");
        h.keys("C-c m");
        assert_eq!(h.core.message(), Some("two"));
    }

    #[test]
    fn invalid_keymap_file_keeps_the_keymaps_in_effect() {
        let mut h = Harness::new(&[("keymap.toml", "[global]\n\"C-c a\" = \"one\"")]);
        echo_commands(&mut h, "C-c z");
        h.keys("C-x C-r");
        std::fs::write(h.path("keymap.toml"), "[global").unwrap();
        h.keys("C-x C-r");
        assert!(h.core.message().unwrap().starts_with("keymap: "));
        h.keys("C-c a");
        assert_eq!(h.core.message(), Some("one"));
        h.core
            .handle(h.now, Msg::KeymapLoaded(Err("permission denied".into())));
        // Appended after the command's message, since no key came between.
        assert_eq!(h.core.message(), Some("one; keymap: permission denied"));
    }

    #[test]
    fn unreadable_settings_file_is_reported() {
        let mut h = Harness::new(&[]);
        h.core
            .handle(h.now, Msg::SettingsLoaded(Err("permission denied".into())));
        assert_eq!(h.core.message(), Some("settings: permission denied"));
        assert_eq!(h.core.settings(), &Settings::default());
    }

    /// Registers `one`, `two` and `three`, which each fail with their name so it shows on the
    /// message line, and binds `keys` to `one` in the global map.
    fn echo_commands(h: &mut Harness, keys: &str) {
        let handlers: [(&'static str, crate::command::Handler); 3] = [
            ("one", |_, _| Err("one".into())),
            ("two", |_, _| Err("two".into())),
            ("three", |_, _| Err("three".into())),
        ];
        for (name, handler) in handlers {
            h.core.register(Command {
                name,
                spec: ArgSpec::default(),
                handler,
            });
        }
        h.core
            .keymaps
            .get_mut("global")
            .unwrap()
            .bind(keys, "one")
            .unwrap();
    }

    #[test]
    fn minor_mode_shadows_global_until_removed() {
        let mut h = Harness::new(&[]);
        echo_commands(&mut h, "C-c a");
        bind_mode(&mut h, "test-mode", "C-c a", "two");
        h.keys("C-c a");
        assert_eq!(h.core.message(), Some("one"));
        buffer(&mut h).minor_modes.push("test-mode");
        h.keys("C-c a");
        assert_eq!(h.core.message(), Some("two"));
        buffer(&mut h).minor_modes.clear();
        h.keys("C-c a");
        assert_eq!(h.core.message(), Some("one"));
        assert_eq!(h.text(), "");
    }

    #[test]
    fn major_mode_sits_between_minor_and_global() {
        let mut h = Harness::new(&[]);
        echo_commands(&mut h, "M-z");
        bind_mode(&mut h, "major", "M-z", "two");
        bind_mode(&mut h, "minor", "M-z", "three");
        buffer(&mut h).major_mode = "major";
        buffer(&mut h).minor_modes.push("minor");
        h.keys("M-z");
        assert_eq!(h.core.message(), Some("three"));
        buffer(&mut h).minor_modes.clear();
        h.keys("M-z");
        assert_eq!(h.core.message(), Some("two"));
    }

    #[test]
    fn mode_prefix_keeps_global_keys_under_it() {
        let mut h = Harness::new(&[("a.txt", "")]);
        h.open("a.txt");
        echo_commands(&mut h, "C-c a");
        bind_mode(&mut h, "test-mode", "C-x a", "two");
        buffer(&mut h).minor_modes.push("test-mode");
        h.keys("x C-x C-s");
        assert_eq!(h.disk("a.txt"), "x\n");
        h.keys("C-x a");
        assert_eq!(h.core.message(), Some("two"));
    }

    #[test]
    fn transient_map_wins_once() {
        let mut h = Harness::new(&[]);
        echo_commands(&mut h, "C-c a");
        let mut transient = crate::keymap::Keymap::default();
        transient.bind("C-c a", "two").unwrap();
        h.core.transient = Some(transient);
        h.keys("C-c a");
        assert_eq!(h.core.message(), Some("two"));
        h.keys("C-c a");
        assert_eq!(h.core.message(), Some("one"));
        // C-g cancelling a prefix ends the sequence, so it takes the transient map too.
        let mut transient = crate::keymap::Keymap::default();
        transient.bind("C-c a", "two").unwrap();
        h.core.transient = Some(transient);
        h.keys("C-c C-g C-c a");
        assert_eq!(h.core.message(), Some("one"));
    }

    #[test]
    fn two_level_prefix_runs_once() {
        let mut h = Harness::new(&[]);
        echo_commands(&mut h, "C-x 4 f");
        h.keys("C-x 4");
        assert_eq!(h.core.message(), None);
        h.keys("f");
        assert_eq!(h.core.message(), Some("one"));
        h.keys("f");
        assert_eq!((h.text().as_str(), h.core.message()), ("f", None));
    }

    #[test]
    fn c_x_hint_lists_its_bindings_after_the_delay() {
        let mut h = Harness::new(&[]);
        h.keys("C-x").advance(999 * MS);
        assert_eq!(h.core.hint(), None, "before the delay");
        h.advance(MS);
        insta::assert_debug_snapshot!(h.core.hint());
    }

    #[test]
    fn hint_marks_prefixes_and_follows_layers() {
        let mut h = Harness::new(&[]);
        echo_commands(&mut h, "C-x 4 f");
        bind_mode(&mut h, "test-mode", "C-x u", "two");
        buffer(&mut h).minor_modes.push("test-mode");
        h.keys("C-x").advance(1000 * MS);
        let entries = h.core.hint().unwrap().entries;
        for entry in [("4", "+prefix"), ("u", "two"), ("C-s", "save-buffer")] {
            let entry = (entry.0.to_string(), entry.1.to_string());
            assert!(entries.contains(&entry), "{entry:?}: {entries:?}");
        }
        h.keys("4").advance(1000 * MS);
        let hint = h.core.hint().unwrap();
        assert_eq!(hint.keys, "C-x 4");
        let f = ("f".to_string(), "one".to_string());
        assert!(hint.entries.contains(&f), "{:?}", hint.entries);
    }

    #[test]
    fn the_next_key_hides_the_hint_and_restarts_the_delay() {
        let mut h = Harness::new(&[]);
        h.keys("C-x").advance(1000 * MS);
        assert!(h.core.hint().is_some());
        h.keys("C-g C-f").advance(1000 * MS);
        assert_eq!(h.core.hint(), None, "C-g hides it and no timer is left");
        h.keys("C-x")
            .advance(600 * MS)
            .keys("C-s")
            .advance(600 * MS);
        assert_eq!(h.core.hint(), None, "a finished sequence cancels the timer");
        assert_eq!(h.core.next_deadline(), None);
    }

    #[test]
    fn undefined_keys_are_reported() {
        let mut h = Harness::new(&[]);
        h.keys("C-x C-q");
        assert_eq!(h.core.message(), Some("C-x C-q is undefined"));
        h.keys("M-z");
        assert_eq!(h.core.message(), Some("M-z is undefined"));
        h.keys("TAB");
        assert_eq!(h.core.message(), Some("TAB is undefined"));
        h.keys("ab");
        assert_eq!((h.text().as_str(), h.core.message()), ("ab", None));
    }

    #[test]
    fn c_g_quits_a_prefix_and_on_its_own() {
        let mut h = Harness::new(&[("a.txt", "")]);
        h.open("a.txt");
        h.keys("x C-x C-g");
        assert_eq!(h.core.message(), Some("Quit"));
        h.keys("C-s");
        assert_eq!(h.core.message(), Some("C-s is undefined"));
        assert_eq!(h.disk("a.txt"), "");
        h.keys("C-g");
        assert_eq!(h.core.message(), Some("Quit"));
    }
}
