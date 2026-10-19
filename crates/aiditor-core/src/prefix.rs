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

#[cfg(test)]
mod tests {
    use crate::command::ArgSpec;
    use crate::command::Args;
    use crate::command::Prefix;
    use crate::harness::Harness;
    use crate::harness::bind_mode;
    use crate::harness::insert_cmd;
    use crate::harness::test_cmd;

    /// Feeds `keys` and returns the message line.
    fn prefix_after(h: &mut Harness, keys: &str) -> Option<String> {
        h.keys(keys);
        h.core.message().map(str::to_owned)
    }

    #[test]
    fn prefix_argument_keys_build_the_argument() {
        let mut h = Harness::new(&[]);
        test_cmd(&mut h, ArgSpec::default());
        bind_mode(&mut h, "global", "C-c p", "test-cmd");
        let shows = |prefix: Option<Prefix>| {
            Some(format!(
                "{:?}",
                Args {
                    prefix,
                    ..Args::default()
                }
            ))
        };
        for (keys, prefix) in [
            ("C-u 3", Prefix::Number(3)),
            ("C-u", Prefix::Universal(4)),
            ("C-u C-u", Prefix::Universal(16)),
            ("C--", Prefix::Minus),
            ("C-u 1 2", Prefix::Number(12)),
            ("C-u - 5", Prefix::Number(-5)),
            ("C-u - 0 5", Prefix::Number(-5)),
            ("M-4 2", Prefix::Number(42)),
            ("M-4 M-2", Prefix::Number(42)),
            ("C-- 7", Prefix::Number(-7)),
            ("C-u C--", Prefix::Minus),
            ("C-u 3 C-u", Prefix::Number(3)),
        ] {
            let got = prefix_after(&mut h, &format!("{keys} C-c p"));
            assert_eq!(got, shows(Some(prefix)), "{keys}");
        }
        assert_eq!(Prefix::Universal(16).value(), 16);
        assert_eq!(Prefix::Minus.value(), -1);
        assert_eq!(Prefix::Number(3).value(), 3);
        // Only the next command gets it, and C-g or typing clears it.
        assert_eq!(prefix_after(&mut h, "C-u 3 C-c p C-c p"), shows(None));
        assert_eq!(prefix_after(&mut h, "C-u 3 C-g C-c p"), shows(None));
        assert_eq!(prefix_after(&mut h, "C-u C-x C-g C-c p"), shows(None));
        assert_eq!(prefix_after(&mut h, "C-u 3 a C-c p"), shows(None));
        // After digits, C-u ends the number: the next digit types that many times.
        h.keys("C-u 3 C-u 5");
        assert!(h.text().ends_with("aaa555"), "{}", h.text());
    }

    #[test]
    fn prefix_argument_keys_keep_the_last_command() {
        let mut h = Harness::new(&[]);
        insert_cmd(&mut h);
        h.keys("C-c t C-u 3");
        assert_eq!(h.core.last_command(), Some("test-insert"));
        h.keys("C-c t");
        assert_eq!(h.core.last_command(), Some("test-insert"));
    }
}
