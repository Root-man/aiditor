//! Moving point and scrolling.

use crate::Ctx;
use crate::command::Prefix;

impl Ctx<'_> {
    /// Moves point to `p` like Emacs's `mouse-set-point`, dropping any pending prefix.
    pub(crate) fn click(&mut self, line: usize, col: usize) {
        let core = &mut *self.core;
        core.pending.clear();
        core.transient = None;
        core.prefix_arg = None;
        if let Some(id) = core.hint_timer.take() {
            core.timers.cancel(id);
        }
        core.hint_shown = false;
        core.message = None;
        // So the next typed char starts a new undo group.
        core.self_inserts = 0;
        core.window.set_active(false);
        core.last_command = Some("mouse-set-point");
        let buffer = core.current();
        // Point's line is drawn scrolled sideways, so its shown columns start at the window's left.
        let point_line = buffer
            .line_col(core.window.point(&core.buffers))
            .ok()
            .map(|(l, _)| l);
        let col = col
            + if point_line == Some(line) {
                core.window.left()
            } else {
                0
            };
        let p = (buffer.char_idx(line, col)).unwrap_or(buffer.len_chars());
        if let Err(err) = self.goto(p) {
            tracing::debug!(%err, "click outside the buffer");
        }
    }

    /// Moves point to `p`, which must be in the buffer.
    pub(crate) fn goto(&mut self, p: usize) -> Result<(), String> {
        let core = &mut *self.core;
        Ok(core.window.set_point(&mut core.buffers, p)?)
    }

    /// Moves point to `target` clamped to the buffer, reporting an end it ran into.
    fn goto_clamped(&mut self, target: i64) -> Result<(), String> {
        let len = self.core.current().len_chars() as i64;
        self.goto(target.clamp(0, len) as usize)?;
        match target {
            t if t < 0 => Err("Beginning of buffer".into()),
            t if t > len => Err("End of buffer".into()),
            _ => Ok(()),
        }
    }

    pub(crate) fn forward_char(&mut self, n: i64) -> Result<(), String> {
        self.goto_clamped((self.core.point() as i64).saturating_add(n))
    }

    /// Moves `n` lines to the goal column, which a run of line motions keeps.
    pub(crate) fn next_line(&mut self, n: i64) -> Result<(), String> {
        let core = &mut *self.core;
        let (line, col) = core
            .current()
            .line_col(core.point())
            .expect("point is in the buffer");
        if !matches!(core.last_command, Some("next-line" | "previous-line")) {
            // shortcut: the goal is in chars, so tabs and wide chars shift it; use display columns once the display renders them.
            core.goal_column = col;
        }
        let buffer = core.current();
        let target = (line as i64).saturating_add(n);
        let last = buffer.len_lines() as i64 - 1;
        if !(0..=last).contains(&target) {
            return self.goto_clamped(if target < 0 { -1 } else { i64::MAX });
        }
        let p = buffer
            .char_idx(target as usize, core.goal_column)
            .expect("target line is in the buffer");
        self.goto(p)
    }

    pub(crate) fn forward_word(&mut self, n: i64) -> Result<(), String> {
        self.goto(self.word_end(n))
    }

    /// Where `n` words on from point end, a word being a run of alphanumerics; backward when `n`
    /// is negative.
    pub(crate) fn word_end(&self, n: i64) -> usize {
        let buffer = self.core.current();
        let (text, len) = (buffer.text(), buffer.len_chars());
        let word = |i: usize| text.char(i).is_alphanumeric();
        let mut p = self.core.point();
        for _ in 0..n.unsigned_abs() {
            if n > 0 {
                while p < len && !word(p) {
                    p += 1;
                }
                while p < len && word(p) {
                    p += 1;
                }
            } else {
                while p > 0 && !word(p - 1) {
                    p -= 1;
                }
                while p > 0 && word(p - 1) {
                    p -= 1;
                }
            }
            if p == 0 || p == len {
                break;
            }
        }
        p
    }

    /// Moves `n - 1` lines, stopping at the first or last, then to that line's start or end.
    pub(crate) fn line_end(&mut self, n: i64, end: bool) -> Result<(), String> {
        let buffer = self.core.current();
        let (line, _) = buffer
            .line_col(self.core.point())
            .expect("point is in the buffer");
        let last = buffer.len_lines() as i64 - 1;
        let line = (line as i64)
            .saturating_add(n.saturating_sub(1))
            .clamp(0, last);
        let col = if end { usize::MAX } else { 0 };
        let p = buffer
            .char_idx(line as usize, col)
            .expect("line is in the buffer");
        self.goto(p)
    }

    /// Goes to the start, or the end, of the buffer; a number N goes N tenths in from that end,
    /// then to the next line's start.
    pub(crate) fn buffer_end(&mut self, prefix: Option<Prefix>, end: bool) -> Result<(), String> {
        if !self.core.window.active() && !matches!(prefix, Some(Prefix::Universal(_))) {
            self.push_mark(false)?;
        }
        let buffer = self.core.current();
        let len = buffer.len_chars() as i64;
        let Some(n) = prefix.filter(|p| !matches!(p, Prefix::Universal(_))) else {
            return self.goto(if end { len as usize } else { 0 });
        };
        let tenths = len.saturating_mul(n.value()) / 10;
        let p = (if end { len - tenths } else { tenths }).clamp(0, len) as usize;
        let (line, _) = buffer.line_col(p).expect("p is in the buffer");
        let next = buffer.char_idx(line + 1, 0).unwrap_or(len as usize);
        self.goto(next)
    }

    /// Scrolls `n` lines down the buffer, up when negative, keeping point in the window.
    pub(crate) fn scroll_lines(&mut self, n: i64) -> Result<(), String> {
        let core = &mut *self.core;
        let rows = core.window.rows();
        let top = core.window.scroll(&core.buffers, rows);
        let buffer = core.current();
        let last = buffer.len_lines() as i64 - 1;
        let new = (top as i64).saturating_add(n).clamp(0, last) as usize;
        if new == top && n != 0 {
            let end = if n > 0 { "End" } else { "Beginning" };
            return Err(format!("{end} of buffer"));
        }
        let (line, _) = buffer
            .line_col(core.point())
            .expect("point is in the buffer");
        let shown = line.clamp(new, new + rows - 1);
        let p = buffer
            .char_idx(shown, 0)
            .expect("shown line is in the buffer");
        core.window.set_top(new);
        if shown != line {
            self.goto(p)?;
        }
        Ok(())
    }

    /// Scrolls a screenful less two lines for no prefix, the other way for a lone minus, or
    /// the prefix's count of lines; `sign` is -1 to scroll up the buffer.
    pub(crate) fn scroll_command(
        &mut self,
        prefix: Option<Prefix>,
        sign: i64,
    ) -> Result<(), String> {
        let page = (self.core.window.rows() as i64 - 2).max(1);
        let n = match prefix {
            None => page,
            Some(Prefix::Minus) => -page,
            Some(p) => p.value(),
        };
        self.scroll_lines(n.saturating_mul(sign))
    }

    /// Puts point's line at a row: from the prefix, or middle, top, bottom on repeated calls.
    pub(crate) fn recenter(&mut self, prefix: Option<Prefix>) {
        let core = &mut *self.core;
        let rows = core.window.rows() as i64;
        // A prefixed call leaves the cycle where it was, as in Emacs.
        let row = match prefix {
            None => {
                let again = core.last_command == Some("recenter-top-bottom");
                core.recenter = if again { (core.recenter + 1) % 3 } else { 0 };
                [rows / 2, 0, rows - 1][core.recenter]
            }
            Some(Prefix::Universal(_)) => rows / 2,
            Some(p) => {
                let n = p.value();
                (if n < 0 { rows + n } else { n }).clamp(0, rows - 1)
            }
        };
        let (line, _) = core
            .current()
            .line_col(core.point())
            .expect("point is in the buffer");
        core.window.set_top(line.saturating_sub(row as usize));
    }
}

#[cfg(test)]
mod tests {
    use crate::Msg;
    use crate::harness::Harness;
    use crate::harness::line_start;
    use crate::harness::mark;
    use crate::harness::on;
    use crate::harness::twenty_lines;
    use crate::window::WindowId;

    #[test]
    fn left_and_right_cross_lines_and_stop_at_the_ends() {
        let mut h = Harness::new(&[]);
        h.keys("ab RET cd <left> <left>");
        assert_eq!(h.point(), 3);
        h.keys("<left>");
        assert_eq!(h.point(), 2);
        h.keys("<right> <right> <right> <right>");
        assert_eq!(h.point(), 5);
        h.keys("<left> <left> <left> <left> <left> <left>");
        assert_eq!(h.point(), 0);
    }

    #[test]
    fn motion_keys_and_arrows_run_motion_commands() {
        let mut h = on("ab\ncd");
        for (keys, name) in [
            ("C-f", "forward-char"),
            ("<right>", "forward-char"),
            ("C-b", "backward-char"),
            ("<left>", "backward-char"),
            ("C-n", "next-line"),
            ("<down>", "next-line"),
            ("C-p", "previous-line"),
            ("<up>", "previous-line"),
            ("M-f", "forward-word"),
            ("M-b", "backward-word"),
            ("C-a", "move-beginning-of-line"),
            ("C-e", "move-end-of-line"),
            ("M-<", "beginning-of-buffer"),
            ("M-S-<", "beginning-of-buffer"),
            ("M->", "end-of-buffer"),
            ("M-S->", "end-of-buffer"),
            ("C-v", "scroll-up-command"),
            ("M-v", "scroll-down-command"),
            ("C-l", "recenter-top-bottom"),
        ] {
            h.keys(keys);
            assert_eq!(h.core.last_command(), Some(name), "{keys}");
        }
        assert!(!h.core.current().modified());
    }

    #[test]
    fn prefix_argument_counts_char_motion() {
        let mut h = on("abcdef");
        h.keys("C-u 3 C-f");
        assert_eq!(h.point(), 3);
        h.keys("M-< C-u C-f");
        assert_eq!(h.point(), 4);
        h.keys("C-f C-- 2 C-f");
        assert_eq!(h.point(), 3);
        h.keys("C-u 2 C-b");
        assert_eq!(h.point(), 1);
    }

    #[test]
    fn char_motion_crosses_lines_and_reports_the_ends() {
        let mut h = on("ab\ncd");
        h.keys("C-e C-f");
        assert_eq!(h.point(), 3);
        h.keys("C-u 5 C-f");
        assert_eq!((h.point(), h.core.message()), (5, Some("End of buffer")));
        h.keys("C-u 9 C-b");
        assert_eq!(
            (h.point(), h.core.message()),
            (0, Some("Beginning of buffer"))
        );
        h.keys("C-f");
        assert_eq!((h.point(), h.core.message()), (1, None));
    }

    #[test]
    fn line_motion_keeps_the_goal_column() {
        let mut h = on("abcd\nx\nwxyz");
        h.keys("C-u 3 C-f C-n");
        assert_eq!(h.point(), 6, "end of x");
        h.keys("C-n");
        assert_eq!(h.point(), 10, "column 3 of wxyz");
        h.keys("C-p C-p");
        assert_eq!(h.point(), 3);
        h.keys("C-n C-b C-n");
        assert_eq!(h.point(), 7, "C-b reset the goal to column 0");
        h.keys("M-< C-u 3 C-f C-u 2 C-n");
        assert_eq!(h.point(), 10, "a count keeps the goal");
    }

    #[test]
    fn line_motion_stops_at_the_ends() {
        let mut h = on("ab\ncd");
        h.keys("C-f C-u 3 C-n");
        assert_eq!((h.point(), h.core.message()), (5, Some("End of buffer")));
        h.keys("M-< C-f C-p");
        assert_eq!(
            (h.point(), h.core.message()),
            (0, Some("Beginning of buffer"))
        );
        h.keys("C-e <down> <up>");
        assert_eq!(h.point(), 2, "arrows are the line commands");
    }

    #[test]
    fn word_motion() {
        let mut h = on("foo, bar baz");
        h.keys("M-f M-f");
        assert_eq!(h.point(), 8);
        h.keys("M-> C-u 2 M-b");
        assert_eq!(h.point(), 5);
        h.keys("C-u 9 M-f");
        assert_eq!((h.point(), h.core.message()), (12, None));
        h.keys("C-- M-f");
        assert_eq!(h.point(), 9, "negative goes back");
        h.keys("C-u 99999999999 M-b");
        assert_eq!(h.point(), 0);
    }

    #[test]
    fn line_ends() {
        let mut h = on("ab\ncde");
        h.keys("C-u 4 C-f C-a");
        assert_eq!(h.point(), 3);
        h.keys("C-e");
        assert_eq!(h.point(), 6);
        h.keys("M-< C-u 2 C-e");
        assert_eq!(h.point(), 6);
        h.keys("C-u 0 C-a");
        assert_eq!(h.point(), 0, "0 is one line up");
        h.keys("C-u 9 C-e");
        assert_eq!((h.point(), h.core.message()), (6, None));
    }

    #[test]
    fn buffer_ends() {
        let mut h = on("ab\ncd");
        h.keys("C-f C-f M->");
        assert_eq!((h.point(), h.core.message()), (5, Some("Mark set")));
        h.keys("M-<");
        assert_eq!(h.point(), 0);
        assert_eq!(mark(&h), (Some(5), false));
        assert_eq!(h.core.window.ring(&h.core.buffers), [2]);
        let mut h = on("ab\ncd");
        h.keys("C-f C-f C-SPC M->");
        assert_eq!(
            (h.point(), mark(&h)),
            (5, (Some(2), true)),
            "an active region keeps its mark"
        );
        assert_eq!(h.core.window.ring(&h.core.buffers), []);
        let mut h = on(&"123456789\n".repeat(10));
        h.keys("C-u 5 M-<");
        assert_eq!(h.point(), 60);
        h.keys("C-u 3 M->");
        assert_eq!(h.point(), 80, "after the line holding char 70");
        h.keys("C-u M->");
        assert_eq!(h.point(), 100, "plain C-u is no count");
        assert_eq!(mark(&h), (Some(60), false), "plain C-u pushes no mark");
    }

    #[test]
    fn scrolling_by_screenfuls() {
        let mut h = twenty_lines();
        h.keys("C-v");
        assert_eq!((h.core.scroll(5), h.point()), (3, line_start(&h, 3)));
        h.keys("M-v");
        assert_eq!((h.core.scroll(5), h.point()), (0, line_start(&h, 3)));
        h.keys("C-u 2 C-v");
        assert_eq!(h.core.scroll(5), 2);
        h.keys("C-- C-v");
        assert_eq!(h.core.scroll(5), 0, "lone minus scrolls a screenful back");
        h.keys("M-v");
        assert_eq!(
            (h.core.scroll(5), h.core.message()),
            (0, Some("Beginning of buffer"))
        );
        h.keys("C-u 99 C-v");
        assert_eq!(h.core.scroll(5), 19);
        assert_eq!(h.point(), line_start(&h, 19));
        h.keys("C-v");
        assert_eq!(
            (h.core.scroll(5), h.core.message()),
            (19, Some("End of buffer"))
        );
        h.keys("C-u 2 M-v");
        assert_eq!((h.core.scroll(5), h.point()), (17, line_start(&h, 19)));
        h.keys("C-u 4 M-v");
        assert_eq!(h.point(), line_start(&h, 17), "point moves up into view");
    }

    #[test]
    fn recentering() {
        let mut h = twenty_lines();
        h.keys("C-u 10 C-n");
        assert_eq!(h.core.scroll(5), 6);
        for top in [8, 10, 6, 8] {
            h.keys("C-l");
            assert_eq!(h.core.scroll(5), top);
        }
        h.keys("C-u 1 C-l");
        assert_eq!(h.core.scroll(5), 9);
        h.keys("C-- C-l");
        assert_eq!(h.core.scroll(5), 6, "minus is the bottom row");
        h.keys("C-l");
        assert_eq!(h.core.scroll(5), 10, "a prefixed C-l keeps the cycle");
        h.keys("C-u C-l");
        assert_eq!(h.core.scroll(5), 8, "plain C-u recenters");
        h.keys("M-< C-l");
        assert_eq!(h.core.scroll(5), 0, "not above the start");
    }

    #[test]
    fn scrolling_follows_point_through_keys() {
        let mut h = Harness::new(&[("a.txt", "0\n1\n2\n3\n4\n5")]);
        h.keys("x");
        h.open("a.txt");
        assert_eq!((h.core.current().name.as_str(), h.point()), ("a.txt", 0));
        h.keys("<down> <down> <down> <down>");
        assert_eq!(h.core.scroll(3), 2);
        h.keys("<up> <up>");
        assert_eq!(h.core.scroll(3), 2);
        h.keys("<up>");
        assert_eq!(h.core.scroll(3), 1);
        // Visiting it again shows the same buffer where it was.
        h.open("a.txt");
        assert_eq!((h.point(), h.core.scroll(3)), (2, 1));
    }

    #[test]
    fn wheel_scrolls_keeping_point_visible() {
        let mut h = twenty_lines();
        h.core.scroll(3);
        let scroll = |lines| Msg::Scroll {
            window: WindowId(0),
            lines,
        };
        h.send(scroll(5));
        assert_eq!((h.core.scroll(3), h.point()), (5, line_start(&h, 5)));
        h.send(scroll(-2));
        assert_eq!((h.core.scroll(3), h.point()), (3, line_start(&h, 5)));
        h.send(scroll(99)).send(scroll(1));
        assert_eq!((h.core.scroll(3), h.core.message()), (19, None));
        h.send(scroll(-99));
        assert_eq!((h.core.scroll(3), h.core.message()), (0, None));
    }

    #[test]
    fn click_moves_point_and_drops_mark_and_prefix() {
        let mut h = on("abc\ndef");
        h.keys("C-SPC C-f");
        let click = |line, col| Msg::Click {
            window: WindowId(0),
            line,
            col,
        };
        h.send(click(1, 1));
        assert_eq!((h.point(), mark(&h).1), (5, false));
        h.send(click(0, 9));
        assert_eq!(h.point(), 3, "past the line's end");
        h.send(click(5, 0));
        assert_eq!(h.point(), 7, "past the buffer's end");
        h.keys("C-x").send(click(0, 0)).keys("u");
        assert_eq!(h.text(), "uabc\ndef", "no prefix pending");
        h.keys("C-u").send(click(1, 0)).keys("x");
        assert_eq!(h.text(), "uabc\nxdef", "no prefix argument");
        h.keys("C-/");
        assert_eq!(
            h.text(),
            "uabc\ndef",
            "typing after a click is its own undo group"
        );
    }
}
