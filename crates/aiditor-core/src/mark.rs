//! The mark and the region.

use crate::Ctx;
use crate::command::Prefix;

impl Ctx<'_> {
    /// Sets the mark at point, pushing the old one; on a repeat, toggles the region instead.
    pub(crate) fn set_mark_command(&mut self, prefix: Option<Prefix>) -> Result<(), String> {
        if matches!(prefix, Some(Prefix::Universal(_))) {
            return self.pop_to_mark();
        }
        let core = &mut *self.core;
        if core.last_command == Some("set-mark-command")
            && core.window.mark(&core.buffers).is_some()
        {
            let active = !core.window.active();
            core.window.set_active(active);
            let what = if active { "activated" } else { "deactivated" };
            core.echo(format!("Mark {what}"));
            return Ok(());
        }
        self.push_mark(true)
    }

    /// Pushes the mark at point and says so.
    pub(crate) fn push_mark(&mut self, activate: bool) -> Result<(), String> {
        let core = &mut *self.core;
        let point = core.point();
        core.window.push_mark(&mut core.buffers, point, activate)?;
        core.echo("Mark set".into());
        Ok(())
    }

    /// Moves point to the mark and the mark to the ring's next one.
    fn pop_to_mark(&mut self) -> Result<(), String> {
        let mark = self.mark()?;
        self.goto(mark)?;
        self.core.window.pop_mark();
        // So a C-SPC right after sets a new mark rather than toggling, as in Emacs.
        self.core.this_command = Some("pop-to-mark-command");
        Ok(())
    }

    pub(crate) fn exchange_point_and_mark(&mut self) -> Result<(), String> {
        let mark = self.mark()?;
        let core = &mut *self.core;
        let point = core.point();
        core.window.set_mark(&mut core.buffers, point)?;
        core.window.set_active(true);
        self.goto(mark)
    }

    pub(crate) fn mark(&self) -> Result<usize, String> {
        let core = &*self.core;
        (core.window.mark(&core.buffers)).ok_or_else(|| "No mark set in this buffer".into())
    }
}
