//! Workspaces: everything for one task, bound to one project, owning its buffers and its
//! window configuration.

use std::collections::HashMap;

use crate::Core;
use crate::window::{Tree, Window, WindowId};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct WorkspaceId(u64);

pub struct Workspace {
    pub id: WorkspaceId,
    pub name: String,
    /// The name of the project it is bound to; `None` only for the launch workspace until #64.
    pub project: Option<String>,
    /// Its window configuration while another workspace is current; `None` while it is current.
    pub(crate) windows: Option<Windows>,
}

/// A window configuration, saved while its workspace is not current; the current one lives in
/// `Core`'s fields of the same names.
pub(crate) struct Windows {
    window: Window,
    selected: WindowId,
    others: HashMap<WindowId, Window>,
    tree: Tree,
}

impl Windows {
    pub(crate) fn windows_mut(&mut self) -> impl Iterator<Item = &mut Window> {
        std::iter::once(&mut self.window).chain(self.others.values_mut())
    }
}

impl Workspace {
    pub(crate) fn new(id: WorkspaceId, name: &str, project: Option<&str>) -> Self {
        Workspace {
            id,
            name: name.into(),
            project: project.map(Into::into),
            windows: None,
        }
    }
}

impl Core {
    /// Makes a workspace bound to `project` current, showing its new `*scratch*` in a single
    /// window; the workspace that was current keeps its windows.
    pub fn new_workspace(&mut self, name: &str, project: &str) -> WorkspaceId {
        let id = WorkspaceId(self.next_workspace);
        self.next_workspace += 1;
        self.workspaces
            .push(Workspace::new(id, name, Some(project)));
        let scratch = self.buffers.create_scratch();
        self.buffers
            .get_mut(scratch)
            .expect("just created")
            .workspace = Some(id);
        let selected = WindowId(self.next_window);
        self.next_window += 1;
        let windows = Windows {
            window: Window::new(&mut self.buffers, scratch),
            selected,
            others: HashMap::new(),
            tree: Tree::Leaf(selected),
        };
        self.show_workspace(id, windows);
        id
    }

    /// Makes workspace `id` current, showing its windows as they were when it was last current.
    pub fn switch_workspace(&mut self, id: WorkspaceId) -> Result<(), String> {
        let at = (self.workspaces.iter().position(|w| w.id == id)).ok_or("No such workspace")?;
        if let Some(windows) = self.workspaces[at].windows.take() {
            self.show_workspace(id, windows);
        }
        Ok(())
    }

    /// Makes `id` current with `windows` on screen, saving the current workspace's windows.
    // shortcut: an open prompt is cancelled; keep it in its workspace once commands can leave the minibuffer.
    fn show_workspace(&mut self, id: WorkspaceId, windows: Windows) {
        self.close_minibuffer();
        let old = Windows {
            window: std::mem::replace(&mut self.window, windows.window),
            selected: std::mem::replace(&mut self.selected, windows.selected),
            others: std::mem::replace(&mut self.others, windows.others),
            tree: std::mem::replace(&mut self.tree, windows.tree),
        };
        let current = self.buffers.workspace;
        let current = (self.workspaces.iter_mut().find(|w| w.id == current))
            .expect("the current workspace is live");
        current.windows = Some(old);
        self.buffers.workspace = id;
        self.buffers.raise(self.window.buffer());
    }

    /// Kills workspace `id`, its windows and every buffer it owns; killing the current one makes
    /// the one after it current, or the one before when it is last.
    // shortcut: modified buffers are killed without asking; #64's kill command must ask first.
    pub fn kill_workspace(&mut self, id: WorkspaceId) -> Result<(), String> {
        let at = (self.workspaces.iter().position(|w| w.id == id)).ok_or("No such workspace")?;
        if self.workspaces.len() == 1 {
            return Err("Attempt to delete the sole workspace".into());
        }
        if self.buffers.workspace == id {
            let next = self
                .workspaces
                .get(at + 1)
                .unwrap_or(&self.workspaces[at - 1]);
            self.switch_workspace(next.id)?;
        }
        let mut killed = self.workspaces.remove(at);
        let windows = killed
            .windows
            .as_mut()
            .expect("a workspace not current saves its windows");
        for window in windows.windows_mut() {
            window.release(&mut self.buffers);
        }
        let owned: Vec<_> = (self.buffers.recent())
            .filter(|(_, b)| b.workspace == Some(id))
            .map(|(b, _)| b)
            .collect();
        for buffer in owned {
            self.kill_buffer(buffer);
        }
        Ok(())
    }

    pub fn workspaces(&self) -> &[Workspace] {
        &self.workspaces
    }

    pub fn workspace(&self) -> WorkspaceId {
        self.buffers.workspace
    }
}

#[cfg(test)]
mod tests {
    use super::WorkspaceId;
    use crate::buffer::BufferId;
    use crate::harness::Harness;
    use crate::harness::ask;
    use crate::harness::name;

    /// The ids and names of the buffers `workspace` owns, most recent first.
    fn owned(h: &Harness, workspace: Option<WorkspaceId>) -> Vec<(BufferId, String)> {
        (h.core.buffers.recent())
            .filter(|(_, b)| b.workspace == workspace)
            .map(|(id, b)| (id, b.name.clone()))
            .collect()
    }

    fn names(h: &Harness) -> Vec<&str> {
        h.core
            .workspaces()
            .iter()
            .map(|w| w.name.as_str())
            .collect()
    }

    #[test]
    fn launch_has_one_unbound_workspace_and_messages_is_global() {
        let mut h = Harness::new(&[]);
        let default = h.core.workspace();
        let w = &h.core.workspaces()[0];
        assert_eq!((names(&h), &w.project), (vec!["default"], &None));
        assert_eq!(owned(&h, Some(default))[0].1, "*scratch*");
        h.keys("C-g");
        assert_eq!(owned(&h, None)[0].1, "*Messages*");
    }

    #[test]
    fn a_new_workspace_is_current_with_its_own_scratch() {
        let mut h = Harness::new(&[]);
        let w = h.core.new_workspace("w", "p");
        assert!(h.core.current().unkillable);
        assert_eq!(h.core.workspace(), w);
        assert_eq!(h.core.workspaces()[1].project.as_deref(), Some("p"));
        assert_eq!((name(&h), h.text()), ("*scratch*<2>", String::new()));
        assert_eq!(h.core.current().workspace, Some(w));
    }

    #[test]
    fn a_file_open_in_another_workspace_gets_a_second_buffer() {
        let mut h = Harness::new(&[("a.txt", "abc")]);
        h.open("a.txt");
        let first = h.core.window.buffer();
        h.open("a.txt");
        assert_eq!(h.core.window.buffer(), first, "reused in one workspace");
        let w = h.core.new_workspace("w", "p");
        h.open("a.txt");
        assert_ne!(h.core.window.buffer(), first);
        assert_eq!(
            (name(&h), h.core.current().workspace),
            ("a.txt<2>", Some(w))
        );
    }

    #[test]
    fn killing_a_buffer_never_shows_another_workspace_s_buffer() {
        let mut h = Harness::new(&[("a.txt", "abc"), ("b.txt", "def")]);
        h.open("a.txt");
        let w = h.core.new_workspace("w", "p");
        // a.txt, shown here until #66, is more recent than w's scratch.
        h.keys("C-x b a.txt RET");
        h.open("b.txt").keys("C-x k RET");
        assert_eq!(
            (name(&h), h.core.current().workspace),
            ("*scratch*<2>", Some(w))
        );
    }

    #[test]
    fn killing_a_workspace_kills_its_buffers_and_nothing_else() {
        let mut h = Harness::new(&[("a.txt", "abc"), ("b.txt", "def")]);
        let default = h.core.workspace();
        h.open("a.txt").keys("x");
        let a = h.core.new_workspace("a", "p");
        h.open("a.txt").open("b.txt").keys("y C-g");
        let b = h.core.new_workspace("b", "q");
        h.open("b.txt");
        let (kept, messages) = (owned(&h, Some(b)), owned(&h, None));
        let kept_default = owned(&h, Some(default));
        assert_eq!(owned(&h, Some(a)).len(), 3);
        h.core.kill_workspace(a).unwrap();
        assert_eq!(names(&h), ["default", "b"]);
        assert!(owned(&h, Some(a)).is_empty(), "a's buffers are gone");
        assert_eq!(h.core.workspace(), b, "another workspace stays current");
        assert_eq!(owned(&h, Some(b)), kept);
        assert_eq!(owned(&h, Some(default)), kept_default);
        assert_eq!(owned(&h, None), messages);
        assert_eq!(h.text(), "def", "b's buffer still shown");
        h.keys("C-x b a.txt RET");
        assert_eq!(h.text(), "xabc", "the other workspace's edit is untouched");
    }

    #[test]
    fn killing_the_current_workspace_selects_its_neighbour() {
        let mut h = Harness::new(&[]);
        let default = h.core.workspace();
        let a = h.core.new_workspace("a", "p");
        let b = h.core.new_workspace("b", "p");
        h.core.kill_workspace(b).unwrap();
        assert_eq!(
            (h.core.workspace(), h.core.current().workspace),
            (a, Some(a))
        );
        let c = h.core.new_workspace("c", "p");
        h.core.kill_workspace(a).unwrap();
        assert_eq!(
            h.core.workspace(),
            c,
            "killing another keeps the current one"
        );
        h.core.kill_workspace(c).unwrap();
        assert_eq!(h.core.workspace(), default, "the one before when last");
        assert_eq!(name(&h), "*scratch*");
    }

    #[test]
    fn the_sole_workspace_cannot_be_killed() {
        let mut h = Harness::new(&[]);
        let default = h.core.workspace();
        let err = h.core.kill_workspace(default);
        assert_eq!(err, Err("Attempt to delete the sole workspace".into()));
        assert_eq!((names(&h), name(&h)), (vec!["default"], "*scratch*"));
    }

    /// Each shown window's buffer name and point in tree order, and the selected one's index.
    fn layout(h: &Harness) -> (Vec<(String, usize)>, usize) {
        let core = &h.core;
        let leaves = core.tree.leaves();
        let shown = leaves.iter().map(|id| {
            let window = core.others.get(id).unwrap_or(&core.window);
            let buffer = core.buffers.get(window.buffer()).unwrap();
            (buffer.name.clone(), window.point(&core.buffers))
        });
        let selected = leaves.iter().position(|&id| id == core.selected);
        (shown.collect(), selected.unwrap())
    }

    fn one(name: &str) -> (Vec<(String, usize)>, usize) {
        (vec![(name.into(), 0)], 0)
    }

    #[test]
    fn switching_away_and_back_restores_splits_buffers_and_points() {
        let mut h = Harness::new(&[("a.txt", "abc"), ("b.txt", "def")]);
        let default = h.core.workspace();
        h.open("a.txt").keys("C-f C-f C-x 2 C-x o");
        h.open("b.txt").keys("C-f");
        let before = layout(&h);
        assert_eq!(before, (vec![("a.txt".into(), 2), ("b.txt".into(), 1)], 1));
        let w = h.core.new_workspace("w", "p");
        assert_eq!(
            layout(&h),
            one("*scratch*<2>"),
            "a new workspace has one window"
        );
        h.core.switch_workspace(default).unwrap();
        assert_eq!(layout(&h), before);
        assert_eq!(name(&h), "b.txt", "the selected window's buffer is current");
        h.core.switch_workspace(w).unwrap();
        assert_eq!(layout(&h), one("*scratch*<2>"));
    }

    #[test]
    fn window_changes_stay_in_their_workspace() {
        let mut h = Harness::new(&[]);
        let default = h.core.workspace();
        h.keys("C-x 2");
        let w = h.core.new_workspace("w", "p");
        h.keys("C-x 3 C-x 3");
        h.core.switch_workspace(default).unwrap();
        assert_eq!(layout(&h).0.len(), 2);
        h.keys("C-x 1");
        h.core.switch_workspace(w).unwrap();
        assert_eq!(layout(&h).0.len(), 3, "w keeps its three windows");
        h.core.switch_workspace(default).unwrap();
        assert_eq!(layout(&h), one("*scratch*"));
    }

    #[test]
    fn points_follow_edits_made_while_away() {
        let mut h = Harness::new(&[("a.txt", "abc")]);
        let default = h.core.workspace();
        h.open("a.txt").keys("C-e");
        h.core.new_workspace("w", "p");
        // Another workspace's buffer, shown here until #66.
        h.keys("C-x b a.txt RET C-a X Y");
        h.core.switch_workspace(default).unwrap();
        assert_eq!((h.text(), h.point()), ("XYabc".into(), 5));
    }

    #[test]
    fn switching_closes_the_minibuffer_and_to_the_current_one_does_nothing() {
        let mut h = Harness::new(&[]);
        let default = h.core.workspace();
        h.keys("C-x 2");
        let w = h.core.new_workspace("w", "p");
        ask(&mut h, &["Text: "]);
        h.keys("C-c p a b");
        h.core.switch_workspace(default).unwrap();
        assert!(h.core.minibuffer().is_none());
        assert_eq!(layout(&h), (vec![("*scratch*".into(), 0); 2], 0));
        h.core.switch_workspace(default).unwrap();
        assert_eq!(layout(&h).0.len(), 2);
        h.core.switch_workspace(w).unwrap();
        assert_eq!(
            layout(&h),
            one("*scratch*<2>"),
            "no minibuffer window saved"
        );
    }

    #[test]
    fn killing_the_current_workspace_shows_the_neighbour_s_windows() {
        let mut h = Harness::new(&[]);
        h.keys("C-x 3");
        let w = h.core.new_workspace("w", "p");
        h.keys("C-x 2");
        h.core.kill_workspace(w).unwrap();
        assert_eq!(layout(&h), (vec![("*scratch*".into(), 0); 2], 0));
        let frame = h.frame(800., 600.);
        assert!(
            frame.windows[0].rect.x < frame.windows[1].rect.x,
            "side by side"
        );
    }

    #[test]
    fn a_killed_buffer_is_replaced_in_a_workspace_that_is_not_current() {
        let mut h = Harness::new(&[("a.txt", "abc"), ("b.txt", "def")]);
        let default = h.core.workspace();
        let w = h.core.new_workspace("w", "p");
        h.open("a.txt");
        h.core.switch_workspace(default).unwrap();
        h.keys("C-x b a.txt RET");
        h.core.switch_workspace(w).unwrap();
        h.keys("C-x k RET");
        h.core.switch_workspace(default).unwrap();
        assert_eq!(layout(&h), one("*scratch*"));
        // The same when its workspace is killed while a third one is current.
        h.open("b.txt").keys("C-x 2");
        h.core.switch_workspace(w).unwrap();
        h.open("b.txt");
        let b = h.core.window.buffer();
        h.core.switch_workspace(default).unwrap();
        h.keys("C-x b b.txt<2> RET");
        h.core.new_workspace("x", "p");
        h.core.kill_workspace(w).unwrap();
        assert!(h.core.buffers.get(b).is_none());
        h.core.switch_workspace(default).unwrap();
        assert_eq!(layout(&h).0[0].0, "b.txt");
    }
}
