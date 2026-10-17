//! Keymaps: tries of key sequences, each bound to a command name or a further prefix.

use std::collections::HashMap;

use crate::key::{Key, parse_keys};

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
