//! Keymaps: tries of key sequences, each bound to a command name or a further prefix.

use std::collections::{HashMap, HashSet};

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
/// A value naming a table and no command binds that table as a named prefix, and `false`
/// unbinds. Only text that is not TOML fails the whole load.
pub fn parse(
    bundled: &[&str],
    user: Option<&str>,
    command: impl Fn(&str) -> Option<&'static str>,
) -> Result<(HashMap<String, Keymap>, Vec<String>), String> {
    let files = bundled
        .iter()
        .copied()
        .chain(user)
        .map(|text| toml::from_str::<toml::Table>(text).map_err(|e| e.to_string()))
        .collect::<Result<Vec<_>, _>>()?;
    let tables: HashSet<&str> = files
        .iter()
        .flatten()
        .filter(|(_, entries)| entries.is_table())
        .map(|(name, _)| name.as_str())
        .collect();
    let mut maps: HashMap<String, Keymap> = HashMap::new();
    let mut errors = Vec::new();
    // Bound in order rather than merged as tables, so a user entry wins however it is spelled.
    for (name, entries) in files.iter().flatten() {
        let Some(entries) = entries.as_table() else {
            errors.push(format!("{name} is not a table of bindings"));
            continue;
        };
        let map = maps.entry(name.clone()).or_default();
        for (keys, value) in entries {
            let bound = match value {
                toml::Value::Boolean(false) => map.unbind(keys),
                toml::Value::String(name) => match command(name) {
                    Some(name) => map.bind(keys, name),
                    None if tables.contains(name.as_str()) => map.bind_prefix(keys, name),
                    None => Err(format!("no command or keymap named {name}")),
                },
                _ => Err(format!("{value} is not a command name")),
            };
            if let Err(err) = bound {
                errors.push(format!("{keys}: {err}"));
            }
        }
    }
    // Filled only now, as a prefix's table may come after the keymaps that bind it.
    let unresolved = maps.clone();
    for (name, map) in &mut maps {
        map.resolve(&unresolved, &mut vec![name.clone()], &mut errors);
    }
    Ok((maps, errors))
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Keymap {
    bindings: HashMap<Key, Binding>,
    /// The table a named prefix was bound from, shown by the hint.
    name: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Binding {
    Command(&'static str),
    Prefix(Keymap),
}

impl Keymap {
    /// Binds the `kbd`-style `keys` to `command`, making prefixes on the way and replacing
    /// whatever was bound there.
    pub fn bind(&mut self, keys: &str, command: &'static str) -> Result<(), String> {
        let (map, last) = self.slot(keys)?;
        map.bindings.insert(last, Binding::Command(command));
        Ok(())
    }

    /// Binds `keys` as a prefix holding the keymap named `name`, filled by `resolve`.
    fn bind_prefix(&mut self, keys: &str, name: &str) -> Result<(), String> {
        let (map, last) = self.slot(keys)?;
        let prefix = Keymap {
            name: Some(name.into()),
            ..Keymap::default()
        };
        map.bindings.insert(last, Binding::Prefix(prefix));
        Ok(())
    }

    /// Removes whatever `keys` is bound to in this map, if anything.
    // shortcut: a key under a named prefix is unbound in the prefix's own table, not through a
    // sequence such as `C-x p f`, which only adds; upgrade if users need per-sequence removal.
    fn unbind(&mut self, keys: &str) -> Result<(), String> {
        let keys = parse_keys(keys)?;
        let (last, prefix) = keys.split_last().ok_or("empty key sequence")?;
        let mut map = self;
        for key in prefix {
            match map.bindings.get_mut(key) {
                Some(Binding::Prefix(next)) => map = next,
                _ => return Ok(()),
            }
        }
        map.bindings.remove(last);
        Ok(())
    }

    /// The map that binds the last key of `keys`, making prefixes on the way, with that key.
    fn slot(&mut self, keys: &str) -> Result<(&mut Keymap, Key), String> {
        let keys = parse_keys(keys)?;
        let (last, prefix) = keys.split_last().ok_or("empty key sequence")?;
        let mut map = self;
        for key in prefix {
            let entry = map
                .bindings
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
        Ok((map, *last))
    }

    /// Fills every named prefix under this map with a copy of its table from `tables`, with
    /// what was bound under the prefix itself laid over it. `stack` holds the tables being
    /// filled, so a prefix inside itself is reported and left as it is.
    fn resolve(
        &mut self,
        tables: &HashMap<String, Keymap>,
        stack: &mut Vec<String>,
        errors: &mut Vec<String>,
    ) {
        for binding in self.bindings.values_mut() {
            let Binding::Prefix(map) = binding else {
                continue;
            };
            map.resolve(tables, stack, errors);
            let Some(name) = map.name.clone() else {
                continue;
            };
            if stack.contains(&name) {
                let err = format!("{name}: prefix cycle");
                if !errors.contains(&err) {
                    errors.push(err);
                }
                continue;
            }
            let mut full = tables.get(&name).cloned().unwrap_or_default();
            stack.push(name);
            full.resolve(tables, stack, errors);
            full.name = stack.pop();
            full.bindings.extend(std::mem::take(&mut map.bindings));
            *map = full;
        }
    }

    /// The name of the table this prefix was bound from, if it was bound by name.
    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    /// Every key this map binds directly, with its binding, in no order.
    pub fn bindings(&self) -> impl Iterator<Item = (&Key, &Binding)> {
        self.bindings.iter()
    }

    /// What `keys` is bound to, or `None` if this map leaves it unbound.
    pub fn lookup(&self, keys: &[Key]) -> Option<&Binding> {
        let (first, rest) = keys.split_first()?;
        match self.bindings.get(first)? {
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
            "no command or keymap named nope",
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

    #[test]
    fn named_prefixes_resolve_under_every_sequence() {
        let bundled = r#"
            [global]
            "C-x p" = "prefix"
            "C-x p z" = "save"
            "C-x u" = "undo"
            [prefix]
            "f" = "undo"
        "#;
        let user = r#"
            [global]
            "C-c p" = "prefix"
            "C-x u" = false
            "C-c q" = "no-such-prefix"
            "C-c l" = "loop"
            [prefix]
            "g" = "save"
            [loop]
            "l" = "loop"
        "#;
        let (maps, errors) = parse(&[bundled], Some(user), known).unwrap();
        let global = &maps["global"];
        for keys in ["C-x p", "C-c p"] {
            let Some(Binding::Prefix(map)) = lookup(global, keys) else {
                panic!("{keys} is not a prefix");
            };
            assert_eq!(map.name(), Some("prefix"));
            assert_eq!(lookup(map, "f"), Some(&Binding::Command("undo")));
            assert_eq!(lookup(map, "g"), Some(&Binding::Command("save")));
        }
        // Bound under one sequence, it stays there.
        assert_eq!(lookup(global, "C-x p z"), Some(&Binding::Command("save")));
        assert_eq!(lookup(global, "C-c p z"), None);
        assert_eq!(lookup(global, "C-x u"), None);
        assert_eq!(errors.len(), 2, "{errors:?}");
        for needle in [
            "C-c q: no command or keymap named no-such-prefix",
            "loop: prefix cycle",
        ] {
            assert!(errors.iter().any(|e| e == needle), "{needle}: {errors:?}");
        }
    }
}
