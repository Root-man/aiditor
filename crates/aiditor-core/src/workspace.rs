//! Workspaces: everything for one task, bound to one project, owning its buffers.

use crate::Core;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct WorkspaceId(u64);

pub struct Workspace {
    pub id: WorkspaceId,
    pub name: String,
    /// The name of the project it is bound to; `None` only for the launch workspace until #64.
    pub project: Option<String>,
}

impl Core {
    /// Makes a workspace bound to `project` current, showing its new `*scratch*` in the selected
    /// window.
    pub fn new_workspace(&mut self, name: &str, project: &str) -> WorkspaceId {
        let id = WorkspaceId(self.next_workspace);
        self.next_workspace += 1;
        self.workspaces.push(Workspace {
            id,
            name: name.into(),
            project: Some(project.into()),
        });
        self.buffers.workspace = id;
        let scratch = self.buffers.create_scratch();
        self.switch_to(scratch);
        id
    }

    /// Kills workspace `id` and every buffer it owns; killing the current one makes the one
    /// after it current, or the one before when it is last.
    // shortcut: modified buffers are killed without asking; #64's kill command must ask first.
    pub fn kill_workspace(&mut self, id: WorkspaceId) -> Result<(), String> {
        let at = (self.workspaces.iter().position(|w| w.id == id)).ok_or("No such workspace")?;
        if self.workspaces.len() == 1 {
            return Err("Attempt to delete the sole workspace".into());
        }
        self.workspaces.remove(at);
        if self.buffers.workspace == id {
            self.buffers.workspace = self.workspaces[at.min(self.workspaces.len() - 1)].id;
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
}
