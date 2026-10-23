//! Inserting, deleting and undoing text.

use std::ops::Range;

use crate::Ctx;

impl Ctx<'_> {
    /// Deletes the UTF-16 `range` of the current buffer and puts point at its start.
    pub(crate) fn replace_utf16(&mut self, range: Range<usize>) -> Result<(), String> {
        let text = self.core.current().text();
        if range.start > range.end || range.end > text.len_utf16_cu() {
            return Err(format!(
                "Input method range {range:?} is outside the buffer"
            ));
        }
        let (start, end) = (
            text.utf16_cu_to_char(range.start),
            text.utf16_cu_to_char(range.end),
        );
        self.goto(start)?;
        self.delete_to(end)
    }

    /// Undoes the current buffer's last group, or redoes the last undone one, and restores point.
    pub(crate) fn undo(&mut self, redo: bool) {
        let core = &mut *self.core;
        let buffer = core
            .buffers
            .get_mut(core.windows.window.buffer())
            .expect("current buffer is live");
        let point = if redo { buffer.redo() } else { buffer.undo() };
        match point {
            Some(p) => {
                if let Err(err) = core.windows.window.set_point(&mut core.buffers, p) {
                    tracing::debug!(%err, "undo left point out of range");
                }
            }
            None => {
                let what = if redo { "redo" } else { "undo" };
                core.echo(format!("No further {what} information"));
            }
        }
    }

    /// Inserts `text` `n` times at point, moving point after it unless `stay`.
    pub(crate) fn insert_n(&mut self, text: &str, n: i64, stay: bool) -> Result<(), String> {
        let n = usize::try_from(n).map_err(|_| format!("Negative repetition argument {n}"))?;
        let point = self.core.point();
        // shortcut: a huge count allocates without limit, as in Emacs; cap it if that bites.
        let text = text.repeat(n);
        self.core.current_mut().insert(point, &text)?;
        if stay {
            return Ok(());
        }
        self.goto(point + text.chars().count())
    }

    /// `delete-char` and `delete-backward-char`: without a prefix an active, non-empty region is
    /// deleted (not killed), as Emacs's `delete-active-region`; otherwise `delete_n`.
    pub(crate) fn delete_char(&mut self, no_prefix: bool, n: i64) -> Result<(), String> {
        let point = self.core.point();
        match self.core.windows.window.mark(&self.core.buffers) {
            Some(mark) if no_prefix && self.core.windows.window.active() && mark != point => {
                self.delete_to(mark)
            }
            _ => self.delete_n(n),
        }
    }

    /// Deletes `n` chars after point, or `-n` before it; one that would pass an end deletes nothing.
    fn delete_n(&mut self, n: i64) -> Result<(), String> {
        let target = (self.core.point() as i64).saturating_add(n);
        if target < 0 {
            return Err("Beginning of buffer".into());
        }
        if target > self.core.current().len_chars() as i64 {
            return Err("End of buffer".into());
        }
        self.delete_to(target as usize)
    }

    /// Deletes between point and `target`; point's marker moves to the range's start.
    pub(crate) fn delete_to(&mut self, target: usize) -> Result<(), String> {
        let point = self.core.point();
        Ok(self
            .core
            .current_mut()
            .delete(point.min(target)..point.max(target))?)
    }
}

#[cfg(test)]
mod tests {
    use crate::Msg;
    use crate::harness::Harness;
    use crate::harness::mark;
    use crate::harness::on;
    use crate::key::Key;
    use crate::key::KeyCode;
    use std::ops::Range;

    #[test]
    fn types_into_a_buffer() {
        let mut h = Harness::new(&[("src/a.txt", "on disk")]);
        assert!(h.core.current().name.starts_with("*scratch*"));
        h.keys("hello RET wörld C-x C-s");
        assert_eq!(h.text(), "hello\nwörld");
        assert_eq!(h.point(), 11);
        assert!(h.core.current().modified());
        assert_eq!(h.core.current().name, "*scratch*<main>");
        assert!(h.core.current().file.is_none());
    }

    #[test]
    fn undo_restores_text_and_point() {
        let mut h = Harness::new(&[]);
        h.keys("abc <left> DEL C-/");
        assert_eq!((h.text().as_str(), h.point()), ("abc", 2));
        let mut h = Harness::new(&[]);
        h.keys("ab RET c C-/ C-_ C-x u");
        assert_eq!((h.text().as_str(), h.point()), ("", 0));
        h.keys("C-? C-? C-?");
        assert_eq!((h.text().as_str(), h.point()), ("ab\nc", 4));
        assert_eq!(h.core.message(), None);
        h.keys("C-?");
        assert_eq!(h.core.message(), Some("No further redo information"));
    }

    #[test]
    fn a_new_edit_clears_redo() {
        let mut h = Harness::new(&[]);
        h.keys("ab C-/ x C-?");
        assert_eq!(h.text(), "x");
        assert_eq!(h.core.message(), Some("No further redo information"));
    }

    #[test]
    fn typing_amalgamates_up_to_twenty_chars() {
        let mut h = Harness::new(&[]);
        h.keys("hello C-/");
        assert_eq!(h.text(), "");
        h.keys("abcdefghijklmnopqrstuvwxy C-/");
        assert_eq!(h.text(), "abcdefghijklmnopqrst");
        let mut h = Harness::new(&[]);
        h.keys("ab <left> <right> cd C-/");
        assert_eq!(h.text(), "ab");
    }

    #[test]
    fn nothing_to_undo() {
        let mut h = Harness::new(&[]);
        h.keys("C-/");
        assert_eq!((h.text().as_str(), h.point()), ("", 0));
        assert_eq!(h.core.message(), Some("No further undo information"));
    }

    #[test]
    fn redo_ignores_shift() {
        let mut h = Harness::new(&[]);
        h.keys("a C-/");
        let key = Key {
            ctrl: true,
            shift: true,
            ..Key::plain(KeyCode::Char('?'))
        };
        h.core.handle(h.now, Msg::Key(key));
        assert_eq!(h.text(), "a");
    }

    #[test]
    fn del_deletes_back_and_stops_at_the_start() {
        let mut h = Harness::new(&[]);
        h.keys("abc DEL");
        assert_eq!((h.text().as_str(), h.point()), ("ab", 2));
        h.keys("DEL DEL DEL");
        assert_eq!((h.text().as_str(), h.point()), ("", 0));
        assert_eq!(h.core.message(), Some("Beginning of buffer"));
        assert!(h.core.current().modified());
        h.keys("abcd C-u 2 DEL");
        assert_eq!((h.text().as_str(), h.point()), ("ab", 2));
    }

    #[test]
    fn editing_keys_run_editing_commands() {
        let mut h = on("ab cd");
        for (keys, name) in [
            ("x", "self-insert-command"),
            ("RET", "newline"),
            ("C-j", "electric-newline-and-maybe-indent"),
            ("C-o", "open-line"),
            ("C-d", "delete-char"),
            ("DEL", "delete-backward-char"),
            // Kills record themselves as kill-region, so the next kill joins them.
            ("M-d", "kill-region"),
            ("M-DEL", "kill-region"),
        ] {
            h.keys(keys);
            assert_eq!(h.core.last_command(), Some(name), "{keys}");
        }
    }

    #[test]
    fn typing_repeats_by_the_prefix_argument() {
        let mut h = Harness::new(&[]);
        h.keys("C-u 3 a");
        assert_eq!((h.text().as_str(), h.point()), ("aaa", 3));
        assert_eq!(h.core.last_command(), Some("self-insert-command"));
        h.keys("C-/");
        assert_eq!(h.text(), "", "one undo group");
        h.keys("C-u RET");
        assert_eq!((h.text().as_str(), h.point()), ("\n\n\n\n", 4));
        h.keys("C-u 0 b");
        assert_eq!(h.text(), "\n\n\n\n");
        h.keys("C-- a");
        assert_eq!(h.text(), "\n\n\n\n");
        assert_eq!(h.core.message(), Some("Negative repetition argument -1"));
    }

    #[test]
    fn newline_keys() {
        let mut h = Harness::new(&[]);
        h.keys("a C-u 2 C-j");
        assert_eq!((h.text().as_str(), h.point()), ("a\n\n", 3));
        let mut h = on("ab");
        h.keys("C-f C-u 2 C-o");
        assert_eq!((h.text().as_str(), h.point()), ("a\n\nb", 1));
        h.keys("C-- C-o");
        assert_eq!(h.core.message(), Some("Negative repetition argument -1"));
    }

    #[test]
    fn delete_char_and_its_ends() {
        let mut h = on("abc");
        h.keys("C-d");
        assert_eq!((h.text().as_str(), h.point()), ("bc", 0));
        let mut h = on("abc");
        h.keys("M-> C-- 2 C-d");
        assert_eq!((h.text().as_str(), h.point()), ("a", 1));
        let mut h = on("abc");
        h.keys("C-u 2 C-f C-u 2 C-d");
        assert_eq!(h.text(), "abc");
        assert_eq!(h.core.message(), Some("End of buffer"));
        h.keys("C-- 3 DEL");
        assert_eq!(
            h.text(),
            "abc",
            "negative DEL deletes forward, past the end"
        );
        assert_eq!(h.core.message(), Some("End of buffer"));
    }

    #[test]
    fn kill_word_keys() {
        let mut h = on("foo bar baz");
        h.keys("M-d");
        assert_eq!((h.text().as_str(), h.point()), (" bar baz", 0));
        let mut h = on("foo bar baz");
        h.keys("C-u 2 M-d");
        assert_eq!(h.text(), " baz");
        let mut h = on("foo bar");
        h.keys("M-> M-DEL");
        assert_eq!((h.text().as_str(), h.point()), ("foo ", 4));
        h.keys("C-- M-d");
        assert_eq!(
            (h.text().as_str(), h.point()),
            ("", 0),
            "negative goes back"
        );
        let mut h = on("foo bar");
        h.keys("M-> M-DEL C-y");
        assert_eq!(h.text(), "foo bar", "the killed word yanks back");
    }

    #[test]
    fn deletes_remove_an_active_region() {
        for keys in ["C-e C-SPC C-a DEL", "C-a C-SPC C-e DEL", "C-SPC C-e C-d"] {
            let mut h = on("hello\nx");
            h.keys(keys);
            assert_eq!((h.text().as_str(), h.point()), ("\nx", 0), "{keys}");
            assert!(!mark(&h).1, "{keys}");
        }
        let mut h = on("hello");
        h.keys("C-SPC C-f C-f C-g DEL");
        assert_eq!(h.text(), "hllo", "an inactive mark deletes one char");
        let mut h = on("hello");
        h.keys("C-SPC C-f C-f C-f C-u 1 DEL");
        assert_eq!(h.text(), "helo", "a prefix deletes chars, not the region");
    }

    fn insert(text: &str, replace: Option<Range<usize>>) -> Msg {
        Msg::Insert {
            text: text.into(),
            replace,
        }
    }

    #[test]
    fn cmd_z_undoes_and_z_types() {
        let mut h = Harness::new(&[]);
        h.keys("a z");
        assert_eq!(h.text(), "az");
        h.keys("s-z");
        assert_eq!(h.text(), "");
    }

    #[test]
    fn input_methods_replace_typed_text() {
        let mut h = Harness::new(&[]);
        h.keys("e").send(insert("é", Some(0..1)));
        assert_eq!((h.text().as_str(), h.point()), ("é", 1));
        h.keys("C-/");
        assert_eq!(h.text(), "", "one undo removes it");
        h.send(insert("😀e", None)).send(insert("é", Some(2..3)));
        assert_eq!(h.text(), "😀é");
        #[allow(
            clippy::reversed_empty_ranges,
            reason = "a reversed range is one of the bad inputs"
        )]
        for bad in [(2..9), (3..2)] {
            h.send(insert("x", Some(bad.clone())));
            assert_eq!(h.text(), "😀é", "{bad:?}");
            assert!(h.core.message().unwrap().contains("outside"), "{bad:?}");
        }
    }

    #[test]
    fn inserted_text_acts_as_typed_keys() {
        let mut h = Harness::new(&[]);
        h.send(insert("ab", None));
        h.keys("C-x").send(insert("u", None));
        assert_eq!(h.text(), "", "C-x u undoes");
        h.keys("C-u").send(insert("3a", None));
        assert_eq!(h.text(), "aaa");
    }
}
