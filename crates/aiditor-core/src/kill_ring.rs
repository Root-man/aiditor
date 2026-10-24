//! Killing and yanking text.

use std::collections::VecDeque;
use std::ops::Range;

use crate::command::Prefix;
use crate::{Ctx, Effect};

/// How many entries the kill ring keeps, Emacs's `kill-ring-max`.
const KILL_RING_MAX: usize = 60;

/// The kill ring and the system clipboard text it shares entries with.
#[derive(Default)]
pub(crate) struct KillRing {
    /// Kills, newest first.
    ring: VecDeque<String>,
    /// The kill the next yank inserts.
    yank_index: usize,
    /// The clipboard's text for the key being handled, when its command asked for it.
    pub(crate) clipboard: Option<String>,
    /// The clipboard text the editor last wrote or took in, so it isn't pushed again.
    clipboard_seen: Option<String>,
}

impl Ctx<'_> {
    /// Saves the text in `range` on the kill ring, deleting it unless `copy`. Right after a
    /// kill it joins the newest entry, in front of it when `backward`.
    fn kill(&mut self, range: Range<usize>, backward: bool, copy: bool) -> Result<(), String> {
        let pushed = self.core.kills.take_clipboard();
        let core = &mut *self.core;
        let text = (core.current().text().get_slice(range.clone()))
            .ok_or_else(|| format!("Region {range:?} is outside the buffer"))?
            .to_string();
        if !copy {
            core.current_mut().delete(range)?;
        }
        match core.kills.ring.front_mut() {
            Some(head) if !pushed && core.last_command == Some("kill-region") => {
                if backward {
                    head.insert_str(0, &text);
                } else {
                    head.push_str(&text);
                }
                core.kills.yank_index = 0;
            }
            _ => core.kills.kill_new(text),
        }
        let head = core.kills.ring[0].clone();
        core.kills.clipboard_seen = Some(head.clone());
        self.effects.push(Effect::SetClipboard(head));
        // Every kill counts as `kill-region` so the next one joins it, as in Emacs.
        if !copy {
            core.this_command = Some("kill-region");
        }
        Ok(())
    }

    /// Kills between point and `target`.
    pub(crate) fn kill_to(&mut self, target: usize) -> Result<(), String> {
        let point = self.core.point();
        self.kill(point.min(target)..point.max(target), target < point, false)
    }

    /// Kills, or with `copy` copies, the region; backward when point is before the mark.
    pub(crate) fn kill_region(
        &mut self,
        region: Option<Range<usize>>,
        copy: bool,
    ) -> Result<(), String> {
        let region = region.expect("the spec asks for the region");
        let backward = self.core.point() == region.start && !region.is_empty();
        self.kill(region, backward, copy)?;
        self.core.windows.window.set_active(false);
        Ok(())
    }

    /// Kills to the end of the line, through the newline if only blanks are left; a prefix N
    /// kills N lines, or back to the start of the line N up when N is 0 or less.
    pub(crate) fn kill_line(&mut self, prefix: Option<Prefix>) -> Result<(), String> {
        let buffer = self.core.current();
        let point = self.core.point();
        let len = buffer.len_chars();
        let last = buffer.len_lines() as i64 - 1;
        let (line, _) = buffer.line_col(point).expect("point is in the buffer");
        let line_start = |l: i64| match l {
            l if l > last => len,
            l => (buffer.char_idx(l.max(0) as usize, 0)).expect("line is in the buffer"),
        };
        let target = match prefix {
            Some(p) => line_start((line as i64).saturating_add(p.value())),
            None if point == len => return Err("End of buffer".into()),
            None => {
                let end = (buffer.char_idx(line, usize::MAX)).expect("line is in the buffer");
                let blank = (point..end).all(|i| matches!(buffer.text().char(i), ' ' | '\t'));
                if blank {
                    line_start(line as i64 + 1)
                } else {
                    end
                }
            }
        };
        self.kill_to(target)
    }

    /// Inserts the kill `n` on from the yank pointer, with the mark at its start; a plain `C-u`
    /// leaves point at the start instead.
    pub(crate) fn yank(&mut self, prefix: Option<Prefix>) -> Result<(), String> {
        self.core.kills.take_clipboard();
        let n = match prefix {
            None | Some(Prefix::Universal(_)) => 0,
            Some(Prefix::Minus) => -2,
            Some(p) => p.value().saturating_sub(1),
        };
        let text = self.core.kills.current_kill(n)?;
        let core = &mut *self.core;
        let start = core.point();
        core.windows
            .window
            .push_mark(&mut core.buffers, start, false)?;
        self.insert_n(&text, 1, false)?;
        if matches!(prefix, Some(Prefix::Universal(_))) {
            self.point_before_yank(start)?;
        }
        Ok(())
    }

    /// Replaces the text just yanked with the kill `n` further round the ring.
    pub(crate) fn yank_pop(&mut self, n: i64) -> Result<(), String> {
        if self.core.last_command != Some("yank") {
            return Err("Previous command was not a yank".into());
        }
        let text = self.core.kills.current_kill(n)?;
        let (mark, point) = (self.mark()?, self.core.point());
        let start = point.min(mark);
        self.core.current_mut().delete(start..point.max(mark))?;
        self.insert_n(&text, 1, false)?;
        if point < mark {
            self.point_before_yank(start)?;
        }
        self.core.this_command = Some("yank");
        Ok(())
    }

    /// Moves the mark to point at the yanked text's end and point to its `start`.
    fn point_before_yank(&mut self, start: usize) -> Result<(), String> {
        let core = &mut *self.core;
        let end = core.point();
        core.windows.window.set_mark(&mut core.buffers, end)?;
        self.goto(start)
    }
}

impl KillRing {
    /// Pushes clipboard text the editor neither wrote nor took in onto the kill ring, saying
    /// whether it did.
    pub(crate) fn take_clipboard(&mut self) -> bool {
        let Some(text) = self.clipboard.take() else {
            return false;
        };
        if text.is_empty() || self.clipboard_seen.as_ref() == Some(&text) {
            return false;
        }
        self.clipboard_seen = Some(text.clone());
        self.kill_new(text);
        true
    }

    /// Puts `text` on the kill ring as a new entry, dropping the oldest past the limit.
    fn kill_new(&mut self, text: String) {
        self.ring.push_front(text);
        self.ring.truncate(KILL_RING_MAX);
        self.yank_index = 0;
    }

    /// Moves the yank pointer `n` round the kill ring and returns that kill.
    pub(crate) fn current_kill(&mut self, n: i64) -> Result<String, String> {
        let len = self.ring.len() as i64;
        if len == 0 {
            return Err("Kill ring is empty".into());
        }
        self.yank_index = (self.yank_index as i64 + n.rem_euclid(len)).rem_euclid(len) as usize;
        Ok(self.ring[self.yank_index].clone())
    }
}

#[cfg(test)]
mod tests {
    use crate::harness::Harness;
    use crate::harness::bind_mode;
    use crate::harness::mark;
    use crate::harness::on;
    use crate::key::parse_keys;

    /// A buffer holding `text` with point at `p`.
    fn at(text: &str, p: usize) -> Harness {
        let mut h = on(text);
        h.core
            .windows
            .window
            .set_point(&mut h.core.buffers, p)
            .unwrap();
        h
    }

    fn ring(h: &Harness) -> Vec<&str> {
        h.core.kills.ring.iter().map(String::as_str).collect()
    }

    #[test]
    fn only_kill_and_yank_keys_want_the_clipboard() {
        let mut h = on("");
        let key = |s: &str| parse_keys(s).unwrap()[0];
        assert!(h.core.wants_clipboard(key("C-y")));
        assert!(h.core.wants_clipboard(key("s-c")));
        assert!(!h.core.wants_clipboard(key("C-f")));
        assert!(!h.core.wants_clipboard(key("M-y")));
        bind_mode(&mut h, "global", "C-x y", "yank");
        h.keys("C-x");
        assert!(
            h.core.wants_clipboard(key("y")),
            "a prefix that ends in yank"
        );
        assert!(!h.core.wants_clipboard(key("C-y")));
    }

    #[test]
    fn kills_yank_back_and_the_ring_keeps_sixty() {
        let mut h = at("foo bar", 0);
        h.keys("M-d M-> C-y");
        assert_eq!(h.text(), " barfoo");
        let mut h = on("");
        for _ in 0..61 {
            h.keys("x C-SPC C-b C-w C-f");
        }
        assert_eq!(h.core.kills.ring.len(), 60);
    }

    #[test]
    fn kill_line() {
        let mut h = at("ab\ncd", 1);
        h.keys("C-k");
        assert_eq!(h.text(), "a\ncd");
        let mut h = at("ab\ncd", 2);
        h.keys("C-k");
        assert_eq!(h.text(), "abcd", "at the end of a line the newline goes");
        let mut h = at("ab \ncd", 2);
        h.keys("C-k");
        assert_eq!(
            h.text(),
            "abcd",
            "only blanks left, so the newline goes too"
        );
        let mut h = at("a\nb\nc", 0);
        h.keys("C-u 2 C-k");
        assert_eq!(h.text(), "c");
        let mut h = at("ab\ncd", 4);
        h.keys("C-u 0 C-k");
        assert_eq!((h.text().as_str(), h.point()), ("ab\nd", 3));
        let mut h = at("ab", 2);
        h.keys("C-k");
        assert_eq!(
            (h.text().as_str(), h.core.message()),
            ("ab", Some("End of buffer"))
        );
        let mut h = at("ab\ncd", 3);
        h.keys("C-u 5 C-k");
        assert_eq!(h.text(), "ab\n", "past the last line kills to the end");
    }

    #[test]
    fn kill_region_and_copy() {
        let mut h = at("abcd", 1);
        h.keys("C-SPC C-f C-f C-w");
        assert_eq!(
            (h.text().as_str(), h.point(), h.core.windows.window.active()),
            ("ad", 1, false)
        );
        let mut h = at("ab", 0);
        h.keys("C-SPC C-e M-w");
        assert_eq!(
            (h.core.windows.window.active(), h.clipboard.as_deref()),
            (false, Some("ab"))
        );
        h.keys("C-y");
        assert_eq!(h.text(), "abab");
        let mut h = on("ab");
        h.keys("C-w");
        assert_eq!(
            h.core.message(),
            Some("The mark is not set now, so there is no region")
        );
    }

    #[test]
    fn consecutive_kills_join() {
        let mut h = at("ab\ncd", 0);
        h.keys("C-k C-k");
        assert_eq!(
            (ring(&h), h.clipboard.as_deref()),
            (vec!["ab\n"], Some("ab\n"))
        );
        h.keys("M-> C-y");
        assert_eq!(h.text(), "cdab\n");
        let mut h = at("foo bar", 4);
        h.keys("M-d M-DEL C-y");
        assert_eq!(h.text(), "foo bar", "killing backward prepends");
        let mut h = at("ab\ncd", 0);
        h.keys("C-k C-f C-k");
        assert_eq!(ring(&h), ["cd", "ab"]);
        let mut h = at("ab\ncd\nef", 0);
        h.keys("C-SPC M-> C-p C-a C-k M-w C-k");
        assert_eq!(
            ring(&h),
            ["\n", "cdab\n"],
            "M-w joins a kill but a kill after it doesn't"
        );
    }

    #[test]
    fn yank_and_its_prefixes() {
        let mut h = at("ab", 0);
        h.keys("C-k C-y C-y");
        assert_eq!((h.text().as_str(), h.point()), ("abab", 4));
        let mut h = at("a\nb", 0);
        h.keys("C-k C-d C-k C-u 2 C-y");
        assert_eq!(h.text(), "a");
        let mut h = at("ab", 0);
        h.keys("C-k C-u C-y");
        assert_eq!(
            (h.text().as_str(), h.point(), mark(&h)),
            ("ab", 0, (Some(2), false))
        );
        let mut h = on("");
        h.keys("C-y");
        assert_eq!(h.core.message(), Some("Kill ring is empty"));
    }

    #[test]
    fn m_y_cycles_after_a_yank() {
        let mut h = at("a\nb", 0);
        h.keys("C-k C-d C-k C-y M-y");
        assert_eq!(h.text(), "a");
        h.keys("M-y");
        assert_eq!((h.text().as_str(), h.point()), ("b", 1), "wraps round");
        h.keys("C-f M-y");
        assert_eq!(
            (h.text().as_str(), h.core.message()),
            ("b", Some("Previous command was not a yank"))
        );
        let mut h = at("a\nb", 0);
        h.keys("C-k C-d C-k C-u C-y M-y");
        assert_eq!(
            (h.text().as_str(), h.point(), mark(&h).0),
            ("a", 0, Some(1))
        );
    }

    #[test]
    fn clipboard_text_from_elsewhere_joins_the_ring() {
        let mut h = on("");
        h.clipboard = Some("xyz".into());
        h.keys("C-y");
        assert_eq!(h.text(), "xyz");
        let mut h = at("ab", 0);
        h.clipboard = Some("xyz".into());
        h.keys("C-k");
        assert_eq!(h.clipboard.as_deref(), Some("ab"));
        h.keys("C-y M-y");
        assert_eq!(h.text(), "xyz");
        let mut h = at("ab", 0);
        h.keys("C-k C-y");
        assert_eq!(ring(&h), ["ab"], "the editor's own text isn't pushed again");
        let mut h = at("ab\ncd", 0);
        h.keys("C-k");
        h.clipboard = Some("xyz".into());
        h.keys("C-k");
        assert_eq!(
            ring(&h),
            ["\n", "xyz", "ab"],
            "a pushed clipboard ends the run"
        );
    }
}
