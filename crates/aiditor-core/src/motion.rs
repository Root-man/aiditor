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
