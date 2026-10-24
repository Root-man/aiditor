//! Keymaps: tries of key sequences, each bound to a command name or a further prefix.

use std::collections::HashMap;

use crate::key::{Key, parse_keys};

/// The bundled keymap files, bound in order: the shared one, then the platform's clipboard keys.
pub const BUNDLED: [&str; 2] = [
    include_str!("../keymap.toml"),
    if cfg!(target_os = "macos") {
        include_str!("../keymap-macos.toml")
    } else {
        include_str!("../keymap-linux.toml")
    },
];

/// Keymaps by name from the `bundled` keymap files with `user`'s entries bound over it, plus
/// why each bad entry was skipped. `command` returns the registered name for a command name.
/// Only text that is not TOML fails the whole load.
pub fn parse(
    bundled: &[&str],
    user: Option<&str>,
    command: impl Fn(&str) -> Option<&'static str>,
) -> Result<(HashMap<String, Keymap>, Vec<String>), String> {
    let mut maps: HashMap<String, Keymap> = HashMap::new();
    let mut errors = Vec::new();
    // Bound in order rather than merged as tables, so a user entry wins however it is spelled.
    for text in bundled.iter().copied().chain(user) {
        let table: toml::Table = toml::from_str(text).map_err(|e| e.to_string())?;
        for (name, entries) in table {
            let Some(entries) = entries.as_table() else {
                errors.push(format!("{name} is not a table of bindings"));
                continue;
            };
            let map = maps.entry(name).or_default();
            for (keys, value) in entries {
                let bound = value
                    .as_str()
                    .ok_or_else(|| format!("{value} is not a command name"))
                    .and_then(|name| command(name).ok_or(format!("no command named {name}")))
                    .and_then(|name| map.bind(keys, name));
                if let Err(err) = bound {
                    errors.push(format!("{keys}: {err}"));
                }
            }
        }
    }
    Ok((maps, errors))
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Keymap(HashMap<Key, Binding>);

#[derive(Clone, Debug, PartialEq)]
pub enum Binding {
    Command(&'static str),
    Prefix(Keymap),
}

impl Keymap {
    /// Binds the `kbd`-style `keys` to `command`, making prefixes on the way and replacing
    /// whatever was bound there.
    pub fn bind(&mut self, keys: &str, command: &'static str) -> Result<(), String> {
        let keys = parse_keys(keys)?;
        let (last, prefix) = keys.split_last().ok_or("empty key sequence")?;
        let mut map = self;
        for key in prefix {
            let entry = map
                .0
                .entry(*key)
                .or_insert_with(|| Binding::Prefix(Keymap::default()));
            if let Binding::Command(_) = entry {
                *entry = Binding::Prefix(Keymap::default());
            }
            let Binding::Prefix(next) = entry else {
                unreachable!("just made a prefix")
            };
            map = next;
        }
        map.0.insert(*last, Binding::Command(command));
        Ok(())
    }

    /// Every key this map binds directly, with its binding, in no order.
    pub fn bindings(&self) -> impl Iterator<Item = (&Key, &Binding)> {
        self.0.iter()
    }

    /// What `keys` is bound to, or `None` if this map leaves it unbound.
    pub fn lookup(&self, keys: &[Key]) -> Option<&Binding> {
        let (first, rest) = keys.split_first()?;
        match self.0.get(first)? {
            binding if rest.is_empty() => Some(binding),
            Binding::Prefix(map) => map.lookup(rest),
            Binding::Command(_) => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lookup<'a>(map: &'a Keymap, keys: &str) -> Option<&'a Binding> {
        map.lookup(&parse_keys(keys).unwrap())
    }

    fn known(name: &str) -> Option<&'static str> {
        ["undo", "save"].into_iter().find(|c| *c == name)
    }

    #[test]
    fn user_entries_bind_over_bundled_and_bad_ones_are_skipped() {
        let bundled = r#"
            [global]
            "C-x u" = "undo"
            "C-x C-s" = "save"
        "#;
        let user = r#"
            bad = 2
            [global]
            "C-x  u" = "save"
            "C-xy" = "undo"
            "C-c a" = "nope"
            "C-c b" = 1
            "C-c c" = "undo"
            [mode]
            "C-c d" = "undo"
        "#;
        let (maps, errors) = parse(&[bundled], Some(user), known).unwrap();
        let global = &maps["global"];
        assert_eq!(lookup(global, "C-x u"), Some(&Binding::Command("save")));
        assert_eq!(lookup(global, "C-x C-s"), Some(&Binding::Command("save")));
        assert_eq!(lookup(global, "C-c c"), Some(&Binding::Command("undo")));
        assert_eq!(
            lookup(&maps["mode"], "C-c d"),
            Some(&Binding::Command("undo"))
        );
        assert_eq!(lookup(global, "C-c a"), None);
        assert_eq!(errors.len(), 4, "{errors:?}");
        for needle in [
            "C-xy",
            "no command named nope",
            "1 is not a command name",
            "bad is not a table",
        ] {
            assert!(
                errors.iter().any(|e| e.contains(needle)),
                "{needle}: {errors:?}"
            );
        }
        assert!(parse(&[bundled], Some("[global"), known).is_err());
    }

    #[test]
    fn binds_nested_prefixes_and_rebinds() {
        let mut map = Keymap::default();
        map.bind("C-x 4 f", "find-other").unwrap();
        map.bind("C-x C-s", "save").unwrap();
        assert!(matches!(lookup(&map, "C-x"), Some(Binding::Prefix(_))));
        assert!(matches!(lookup(&map, "C-x 4"), Some(Binding::Prefix(_))));
        assert_eq!(
            lookup(&map, "C-x 4 f"),
            Some(&Binding::Command("find-other"))
        );
        assert_eq!(lookup(&map, "C-x C-s"), Some(&Binding::Command("save")));
        assert_eq!(lookup(&map, "C-x 5"), None);
        assert_eq!(lookup(&map, "C-x C-s a"), None);
        // A command over a prefix replaces it, and a prefix over a command replaces that.
        map.bind("C-x 4", "four").unwrap();
        assert_eq!(lookup(&map, "C-x 4 f"), None);
        map.bind("C-x 4 g", "g").unwrap();
        assert_eq!(lookup(&map, "C-x 4 g"), Some(&Binding::Command("g")));
        assert!(map.bind("", "nothing").is_err());
    }
}
