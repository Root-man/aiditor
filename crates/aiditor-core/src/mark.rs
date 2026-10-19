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

#[cfg(test)]
mod tests {
    use crate::harness::Harness;
    use crate::harness::mark;
    use crate::harness::on;

    #[test]
    fn c_spc_sets_the_mark_and_repeats_toggle_it() {
        let mut h = on("hello");
        h.keys("C-SPC");
        assert_eq!(h.core.message(), Some("Mark set"));
        h.keys("C-f C-f");
        assert_eq!((h.point(), mark(&h)), (2, (Some(0), true)));
        h.keys("C-f C-SPC");
        assert_eq!(mark(&h), (Some(3), true));
        assert_eq!(
            h.core.window.ring(&h.core.buffers),
            [0],
            "the old mark is pushed"
        );
        h.keys("C-SPC");
        assert_eq!(
            (mark(&h), h.core.message()),
            ((Some(3), false), Some("Mark deactivated"))
        );
        h.keys("C-SPC");
        assert_eq!(
            (mark(&h), h.core.message()),
            ((Some(3), true), Some("Mark activated"))
        );
    }

    #[test]
    fn c_u_c_spc_pops_the_mark() {
        let mut h = on("hello");
        h.keys("C-u C-SPC");
        assert_eq!(
            (h.point(), h.core.message()),
            (0, Some("No mark set in this buffer"))
        );
        h.keys("C-f C-SPC C-f C-f C-SPC C-e C-u C-SPC");
        assert_eq!((h.point(), mark(&h)), (3, (Some(1), false)));
        assert_eq!(h.core.window.ring(&h.core.buffers), [3]);
        h.keys("C-SPC");
        assert_eq!(mark(&h), (Some(3), true), "C-SPC after a pop sets the mark");
    }

    #[test]
    fn c_x_c_x_exchanges_point_and_mark() {
        let mut h = on("hello");
        h.keys("C-x C-x");
        assert_eq!(
            (h.point(), h.core.message()),
            (0, Some("No mark set in this buffer"))
        );
        h.keys("C-f C-SPC C-SPC C-f C-f C-f C-x C-x");
        assert_eq!((h.point(), mark(&h)), (1, (Some(4), true)));
    }

    #[test]
    fn editing_deactivates_the_mark() {
        let mut h = on("hello");
        h.keys("C-SPC C-f x");
        assert_eq!(mark(&h), (Some(0), false));
        assert_eq!(h.display(1).lines[0].spans.len(), 1, "no region shown");
        h.keys("C-x C-x C-/");
        assert!(!h.core.window.active(), "undo is a change too");
    }

    #[test]
    fn visiting_a_file_drops_the_mark_and_ring() {
        let mut h = Harness::new(&[("a.txt", "abc")]);
        h.keys("x C-SPC C-SPC C-SPC");
        h.open("a.txt");
        assert_eq!(mark(&h), (None, false));
        assert_eq!(h.core.window.ring(&h.core.buffers), []);
    }

    #[test]
    fn region_shows_until_c_g() {
        let mut h = on("hello");
        h.keys("C-SPC C-f C-f");
        insta::assert_debug_snapshot!(h.display(1).lines);
        h.keys("C-g");
        assert_eq!(
            (mark(&h), h.core.message()),
            ((Some(0), false), Some("Quit"))
        );
        insta::assert_debug_snapshot!(h.display(1).lines);
    }
}
