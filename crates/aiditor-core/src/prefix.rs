//! The prefix argument.

use crate::Ctx;
use crate::command::Prefix;
use crate::keymap::Keymap;

impl Ctx<'_> {
    /// Leaves `prefix` for the next command, and the keys that extend it for the next key.
    pub(crate) fn set_prefix(&mut self, prefix: Prefix) {
        let mut map = Keymap::default();
        for digit in 0..10 {
            map.bind(&digit.to_string(), "digit-argument")
                .expect("digits parse");
        }
        map.bind("C-u", "universal-argument").expect("C-u parses");
        if !matches!(prefix, Prefix::Number(_)) {
            map.bind("-", "negative-argument").expect("- parses");
        }
        self.core.prefix_arg = Some(prefix);
        self.core.transient = Some(map);
    }
}
