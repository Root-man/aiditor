//! Killing and yanking text.

use std::ops::Range;

use crate::command::Prefix;
use crate::{Core, Ctx, Effect};

/// How many entries the kill ring keeps, Emacs's `kill-ring-max`.
const KILL_RING_MAX: usize = 60;

impl Ctx<'_> {
    /// Pushes clipboard text the editor neither wrote nor took in onto the kill ring, saying
    /// whether it did.
    fn take_clipboard(&mut self) -> bool {
        let core = &mut *self.core;
        let Some(text) = core.clipboard.take() else {
            return false;
        };
        if text.is_empty() || core.clipboard_seen.as_ref() == Some(&text) {
            return false;
        }
        core.clipboard_seen = Some(text.clone());
        core.kill_new(text);
        true
    }

    /// Saves the text in `range` on the kill ring, deleting it unless `copy`. Right after a
    /// kill it joins the newest entry, in front of it when `backward`.
    fn kill(&mut self, range: Range<usize>, backward: bool, copy: bool) -> Result<(), String> {
        let pushed = self.take_clipboard();
        let core = &mut *self.core;
        let text = (core.current().text().get_slice(range.clone()))
            .ok_or_else(|| format!("Region {range:?} is outside the buffer"))?
            .to_string();
        if !copy {
            core.current_mut().delete(range)?;
        }
        match core.kill_ring.front_mut() {
            Some(head) if !pushed && core.last_command == Some("kill-region") => {
                if backward {
                    head.insert_str(0, &text);
                } else {
                    head.push_str(&text);
                }
                core.yank_index = 0;
            }
            _ => core.kill_new(text),
        }
        let head = core.kill_ring[0].clone();
        core.clipboard_seen = Some(head.clone());
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
        self.core.window.set_active(false);
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
        self.take_clipboard();
        let n = match prefix {
            None | Some(Prefix::Universal(_)) => 0,
            Some(Prefix::Minus) => -2,
            Some(p) => p.value().saturating_sub(1),
        };
        let text = self.core.current_kill(n)?;
        let core = &mut *self.core;
        let start = core.point();
        core.window.push_mark(&mut core.buffers, start, false)?;
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
        let text = self.core.current_kill(n)?;
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
        core.window.set_mark(&mut core.buffers, end)?;
        self.goto(start)
    }
}

impl Core {
    /// Puts `text` on the kill ring as a new entry, dropping the oldest past the limit.
    fn kill_new(&mut self, text: String) {
        self.kill_ring.push_front(text);
        self.kill_ring.truncate(KILL_RING_MAX);
        self.yank_index = 0;
    }

    /// Moves the yank pointer `n` round the kill ring and returns that kill.
    fn current_kill(&mut self, n: i64) -> Result<String, String> {
        let len = self.kill_ring.len() as i64;
        if len == 0 {
            return Err("Kill ring is empty".into());
        }
        self.yank_index = (self.yank_index as i64 + n.rem_euclid(len)).rem_euclid(len) as usize;
        Ok(self.kill_ring[self.yank_index].clone())
    }
}
