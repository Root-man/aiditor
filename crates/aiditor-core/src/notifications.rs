//! System notifications: the core asks for them with `Effect::Notify` and gets clicks back.

use crate::buffer::BufferId;
use crate::{Ctx, Effect};

impl Ctx<'_> {
    /// Asks the frontend to post a notification about `buffer`, unless the settings turn them off.
    pub(crate) fn notify(&mut self, buffer: BufferId, title: String, body: String) {
        if self.core.settings.notifications {
            let tag = buffer.number().to_string();
            self.effects.push(Effect::Notify { tag, title, body });
        }
    }

    /// Notifies about `buffer` unless you can see it: the window is active and the current
    /// workspace shows it.
    pub(crate) fn notify_unseen(&mut self, buffer: BufferId, title: String, body: String) {
        if !self.core.window_active || !self.core.shown_buffers().contains(&buffer) {
            self.notify(buffer, title, body);
        }
    }

    /// Shows the clicked notification's buffer in its workspace; a buffer gone since does nothing.
    pub(crate) fn notification_clicked(&mut self, tag: &str) {
        let core = &mut *self.core;
        let Some((id, workspace)) = (core.buffers.recent())
            .find(|(id, _)| id.number().to_string() == tag)
            .map(|(id, b)| (id, b.workspace))
        else {
            return;
        };
        core.close_minibuffer();
        if let Some(workspace) = workspace.filter(|&w| core.workspace() != Some(w))
            && core.switch_workspace(workspace).is_err()
        {
            return;
        }
        core.switch_to(id);
    }
}

#[cfg(test)]
mod tests {
    use crate::harness::Harness;
    use crate::{Effect, Msg};

    #[test]
    fn the_setting_turns_them_off() {
        let mut h = Harness::new(&[]);
        for on in [true, false] {
            h.core.settings.notifications = on;
            let mut ctx = h.ctx();
            let id = ctx.core.windows.window.buffer();
            ctx.notify(id, "title".into(), "body".into());
            let posted = matches!(&ctx.effects[..], [Effect::Notify { tag, .. }]
                if *tag == id.number().to_string());
            assert_eq!(posted, on);
            assert_eq!(ctx.effects.len(), usize::from(on));
        }
    }

    #[test]
    fn the_window_starts_active_and_follows_the_frontend() {
        let mut h = Harness::new(&[]);
        assert!(h.core.window_active);
        h.send(Msg::WindowActive(false));
        assert!(!h.core.window_active);
        h.send(Msg::WindowActive(true));
        assert!(h.core.window_active);
    }

    #[test]
    fn a_click_shows_the_buffer_in_its_workspace() {
        let mut h = Harness::new(&[("a.txt", "a")]);
        h.open("a.txt");
        let (one, a) = (h.core.workspace().unwrap(), h.core.windows.window.buffer());
        h.new_workspace("two", "home");
        h.send(Msg::NotificationClicked {
            tag: a.number().to_string(),
        });
        assert_eq!(h.core.workspace(), Some(one));
        assert_eq!(h.core.windows.window.buffer(), a);
    }

    #[test]
    fn a_click_on_a_gone_buffer_or_a_bad_tag_does_nothing() {
        let mut h = Harness::new(&[("a.txt", "a")]);
        h.open("a.txt");
        let a = h.core.windows.window.buffer();
        let two = h.new_workspace("two", "home");
        let shown = h.core.windows.window.buffer();
        h.core.switch_workspace(h.core.workspaces()[0].id).unwrap();
        h.keys("C-x k RET");
        assert!(h.core.buffers.get(a).is_none());
        h.core.switch_workspace(two).unwrap();
        for tag in [a.number().to_string(), "x".into()] {
            h.send(Msg::NotificationClicked { tag });
            assert_eq!(h.core.workspace(), Some(two));
            assert_eq!(h.core.windows.window.buffer(), shown);
        }
    }
}
