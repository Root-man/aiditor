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
            .get_mut(core.window.buffer())
            .expect("current buffer is live");
        let point = if redo { buffer.redo() } else { buffer.undo() };
        match point {
            Some(p) => {
                if let Err(err) = core.window.set_point(&mut core.buffers, p) {
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
        match self.core.window.mark(&self.core.buffers) {
            Some(mark) if no_prefix && self.core.window.active() && mark != point => {
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
