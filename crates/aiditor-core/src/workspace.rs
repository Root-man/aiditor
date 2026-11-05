//! Workspaces: everything for one task, bound to one project, owning its buffers and its
//! window configuration.

use std::cmp::Reverse;
use std::collections::HashMap;
use std::path::Path;

use crate::buffer::{BufferId, MarkerId, scratch_name, unique};
use crate::command::Args;
use crate::project::ProjectId;
use crate::window::{Tree, Window, WindowId};
use crate::{Core, Ctx};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct WorkspaceId(u64);

pub struct Workspace {
    pub id: WorkspaceId,
    pub name: String,
    /// The project it is bound to.
    pub project: ProjectId,
    /// When it was last shown, counting shows; the highest is the most recent.
    pub(crate) shown: u64,
    /// Its window configuration while another workspace is current; `None` while it is current.
    pub(crate) windows: Option<Windows>,
    /// Positions jumps left, newest last.
    pub(crate) back: Vec<(BufferId, MarkerId)>,
    /// Positions `xref-go-back` left, newest last.
    pub(crate) forward: Vec<(BufferId, MarkerId)>,
}

/// The active or saved window configuration of a workspace.
pub(crate) struct Windows {
    pub(crate) window: Window,
    pub(crate) selected: WindowId,
    pub(crate) others: HashMap<WindowId, Window>,
    pub(crate) tree: Tree,
}

impl Windows {
    pub(crate) fn windows_mut(&mut self) -> impl Iterator<Item = &mut Window> {
        std::iter::once(&mut self.window).chain(self.others.values_mut())
    }

    pub(crate) fn replace_buffer(
        &mut self,
        buffers: &mut crate::buffer::Buffers,
        killed: BufferId,
        replacement: BufferId,
    ) {
        for window in self.windows_mut().filter(|w| w.buffer() == killed) {
            window.show(buffers, replacement);
        }
    }
}

impl Workspace {
    pub(crate) fn new(id: WorkspaceId, name: &str, project: ProjectId) -> Self {
        Workspace {
            id,
            name: name.into(),
            project,
            shown: 0,
            windows: None,
            back: Vec::new(),
            forward: Vec::new(),
        }
    }
}

impl Core {
    /// Makes a workspace bound to `project` current, named `name` or `name<2>`, ... when that is
    /// taken, showing its new `*scratch*<name>` in a single window; the workspace that was current
    /// keeps its windows.
    pub fn new_workspace(&mut self, name: &str, project: ProjectId) -> WorkspaceId {
        let id = WorkspaceId(self.next_workspace);
        self.next_workspace += 1;
        let name = unique(name, |n| self.workspaces.iter().any(|w| w.name == n));
        self.workspaces.push(Workspace::new(id, &name, project));
        let scratch = self.buffers.create_scratch(&name);
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
        self.show_workspace(Some(id), windows);
        id
    }

    /// Makes workspace `id` current, showing its windows as they were when it was last current.
    pub fn switch_workspace(&mut self, id: WorkspaceId) -> Result<(), String> {
        let at = (self.workspaces.iter().position(|w| w.id == id)).ok_or("No such workspace")?;
        if let Some(windows) = self.workspaces[at].windows.take() {
            self.show_workspace(Some(id), windows);
        }
        Ok(())
    }

    /// Makes `id` current with `windows` on screen, saving the current workspace's windows; `None`
    /// is the lobby. Leaving the lobby kills its list.
    // shortcut: an open prompt is cancelled; keep it in its workspace once commands can leave the minibuffer.
    fn show_workspace(&mut self, id: Option<WorkspaceId>, windows: Windows) {
        self.pending_visit = None;
        self.close_minibuffer();
        let mut old = std::mem::replace(&mut self.windows, windows);
        let current = self.buffers.workspace;
        match (self.workspaces.iter_mut()).find(|w| Some(w.id) == current) {
            Some(current) => current.windows = Some(old),
            // The lobby's windows belong to no workspace.
            None => old.windows_mut().for_each(|w| w.release(&mut self.buffers)),
        }
        if let Some(id) = id {
            if current.is_none() {
                let lists: Vec<_> = (self.buffers.recent())
                    .filter(|(_, b)| b.workspace.is_none() && b.workspace_list.is_some())
                    .map(|(b, _)| b)
                    .collect();
                for list in lists {
                    _ = self.buffers.kill(list);
                }
            }
            self.shown += 1;
            let shown = self.shown;
            let new = (self.workspaces.iter_mut().find(|w| w.id == id)).expect("shown is live");
            new.shown = shown;
        }
        self.buffers.workspace = id;
        self.buffers.raise(self.windows.window.buffer());
    }

    /// Shows the lobby: no current workspace, and one window on a fresh `*Workspaces*` that
    /// belongs to none.
    pub(crate) fn show_lobby(&mut self) {
        let messages = self.messages();
        let selected = WindowId(self.next_window);
        self.next_window += 1;
        let windows = Windows {
            window: Window::new(&mut self.buffers, messages),
            selected,
            others: HashMap::new(),
            tree: Tree::Leaf(selected),
        };
        self.show_workspace(None, windows);
        self.list_workspaces().expect("a new list has its point");
    }

    /// Kills workspace `id`, its windows and every buffer it owns; killing the current one makes
    /// the one after it current, or the one before when it is last, or shows the lobby when it
    /// is the only one.
    pub fn kill_workspace(&mut self, id: WorkspaceId) -> Result<(), String> {
        let at = (self.workspaces.iter().position(|w| w.id == id)).ok_or("No such workspace")?;
        if self.buffers.workspace == Some(id) {
            let next = (self.workspaces.get(at + 1))
                .or_else(|| self.workspaces.get(at.checked_sub(1)?))
                .map(|w| w.id);
            match next {
                Some(next) => self.switch_workspace(next)?,
                None => self.show_lobby(),
            }
        }
        let mut killed = self.workspaces.remove(at);
        let windows = killed
            .windows
            .as_mut()
            .expect("a workspace not current saves its windows");
        for window in windows.windows_mut() {
            window.release(&mut self.buffers);
        }
        let jumps = killed.back.drain(..).chain(killed.forward.drain(..));
        crate::jump::release(&mut self.buffers, jumps);
        let owned: Vec<_> = (self.buffers.recent())
            .filter(|(_, b)| b.workspace == Some(id))
            .map(|(b, _)| b)
            .collect();
        for buffer in owned {
            self.kill_buffer(buffer);
        }
        self.release_projects();
        Ok(())
    }

    /// Leaves the lobby for a workspace, named after it, on the project `path` belongs to, or
    /// else on an ad-hoc one at `root`.
    pub fn launch(&mut self, path: &Path, root: &Path) {
        assert!(self.workspaces.is_empty(), "launch leaves the lobby");
        let project = self.project_for(path, root);
        let name = self.project_name(project).to_string();
        self.new_workspace(&name, project);
    }

    /// The current workspace; `No workspace` in the lobby.
    pub(crate) fn current_workspace(&self) -> Result<&Workspace, String> {
        let current = self.workspace().ok_or("No workspace")?;
        Ok((self.workspaces.iter().find(|w| w.id == current)).expect("current is live"))
    }

    pub fn workspaces(&self) -> &[Workspace] {
        &self.workspaces
    }

    /// The current workspace; `None` in the lobby.
    pub fn workspace(&self) -> Option<WorkspaceId> {
        self.buffers.workspace
    }

    /// The workspaces' names, most recently shown first, with the current one first or last.
    pub(crate) fn workspace_names(&self, current_first: bool) -> Vec<String> {
        let mut workspaces: Vec<&Workspace> = self.workspaces.iter().collect();
        let current = self.workspace();
        workspaces.sort_by_key(|w| ((Some(w.id) == current) != current_first, Reverse(w.shown)));
        workspaces.into_iter().map(|w| w.name.clone()).collect()
    }

    fn workspace_named(&self, name: &str) -> Result<WorkspaceId, String> {
        (self.workspaces.iter().find(|w| w.name == name))
            .map(|w| w.id)
            .ok_or_else(|| format!("No workspace named {name}"))
    }

    /// Makes a new workspace for the project `project`, named `name`, or after the project when
    /// `name` is blank.
    pub(crate) fn workspace_for(&mut self, id: ProjectId, name: &str) -> Result<(), String> {
        self.project_at(id)?;
        let project = self.project_name(id).to_string();
        if name.trim().is_empty() {
            self.new_workspace(&project, id);
            return Ok(());
        }
        if self.workspaces.iter().any(|w| w.name == name) {
            return Err(format!("Workspace {name} already exists"));
        }
        self.new_workspace(name, id);
        Ok(())
    }

    pub(crate) fn switch_to_workspace(&mut self, name: &str) -> Result<(), String> {
        let id = self.workspace_named(name)?;
        self.switch_workspace(id)
    }

    /// Shows the workspace `n` after the current one in creation order, wrapping; before it
    /// when negative.
    pub(crate) fn cycle_workspace(&mut self, n: i64) -> Result<(), String> {
        let current = self.workspace().ok_or("No workspace")?;
        let len = self.workspaces.len() as i64;
        let at = (self.workspaces.iter().position(|w| w.id == current)).expect("current is live");
        let to = (at as i64 + n % len).rem_euclid(len) as usize;
        self.switch_workspace(self.workspaces[to].id)
    }

    pub(crate) fn rename_workspace(&mut self, name: &str) -> Result<(), String> {
        if name.trim().is_empty() {
            return Err("Empty workspace name".into());
        }
        let current = self.workspace().ok_or("No workspace")?;
        if (self.workspaces.iter()).any(|w| w.name == name && w.id != current) {
            return Err(format!("Workspace {name} already exists"));
        }
        let workspace =
            (self.workspaces.iter_mut().find(|w| w.id == current)).expect("current is live");
        workspace.name = name.into();
        let scratch = (self.buffers.recent())
            .find(|(_, b)| b.unkillable && b.workspace == Some(current))
            .map(|(id, _)| id);
        if let Some(scratch) = scratch {
            self.buffers.rename(scratch, &scratch_name(name));
        }
        Ok(())
    }

    /// Shows the project `name`'s most recently shown workspace, or a new one when it has none.
    pub(crate) fn switch_project(&mut self, project: ProjectId) -> Result<(), String> {
        self.project_at(project)?;
        let latest = (self.workspaces.iter())
            .filter(|w| w.project == project)
            .max_by_key(|w| w.shown);
        match latest.map(|w| w.id) {
            Some(id) => self.switch_workspace(id),
            None => self.workspace_for(project, ""),
        }
    }
}

impl Ctx<'_> {
    /// Kills the workspace named by the first string once its modified file buffers are saved
    /// or confirmed, asking as `C-x C-c` does.
    pub(crate) fn kill_workspace(&mut self, mut args: Args) -> Result<(), String> {
        let id = self.core.workspace_named(&args.strings[0])?;
        let processes = self.process_answer(&mut args);
        let mine = |b: &crate::buffer::Buffer| b.workspace == Some(id);
        let Some(saving) = self.save_some("workspace-kill", args.clone(), 1, mine, "kill")? else {
            return Ok(());
        };
        let live = self.core.buffers.recent().any(|(buffer, b)| {
            b.workspace == Some(id)
                && (self.core.live_terminal(buffer) || self.core.live_agent(buffer))
        });
        let answered = processes.is_some();
        if !self.confirm_processes("workspace-kill", args, live, processes, "the workspace")? {
            if !answered {
                return Ok(());
            }
            return saving.iter().try_for_each(|&buffer| self.save(buffer));
        }
        for &buffer in &saving {
            self.save(buffer)?;
        }
        if saving.is_empty() {
            return self.core.kill_workspace(id);
        }
        (self.core.quitting, self.core.then_kill) = (saving, Some(id));
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::WorkspaceId;
    use crate::Msg;
    use crate::buffer::BufferId;
    use crate::harness::Harness;
    use crate::harness::ask;
    use crate::harness::listed;
    use crate::harness::name;
    use crate::project::{Loaded, Project};

    /// The ids and names of the buffers `workspace` owns, most recent first.
    fn owned(h: &Harness, workspace: Option<WorkspaceId>) -> Vec<(BufferId, String)> {
        (h.core.buffers.recent())
            .filter(|(_, b)| b.workspace == workspace)
            .map(|(id, b)| (id, b.name.clone()))
            .collect()
    }

    /// Shows buffer `name` here even when another workspace owns it, which `C-x b` won't.
    fn show(h: &mut Harness, name: &str) {
        h.core.switch_to(h.core.buffers.find(name).unwrap());
    }

    fn names(h: &Harness) -> Vec<&str> {
        h.core
            .workspaces()
            .iter()
            .map(|w| w.name.as_str())
            .collect()
    }

    #[test]
    fn launch_is_the_lobby_and_messages_is_global() {
        let core = crate::Core::default();
        assert!(core.workspaces().is_empty() && core.projects().is_empty());
        assert_eq!(core.workspace(), None);
        assert_eq!(
            (core.current().name.as_str(), core.current().workspace),
            ("*Workspaces*", None)
        );
        // Loaded projects show in the lobby list, and no `$HOME` one.
        let mut h = lobby_projects();
        let frame = h.frame(800., 600.);
        assert!(frame.workspaces.is_empty(), "no workspace in the modeline");
        assert_eq!(
            frame.title, "*Workspaces*",
            "no workspace or project in the title"
        );
        let mode_line = frame.windows[0].display.mode_line.text();
        assert!(mode_line.starts_with("*Workspaces* │") && !mode_line.contains("unwatched"));
        let list = h.core.text().to_string();
        assert!(
            list.contains("app") && list.contains("no workspaces"),
            "{list}"
        );
        assert_eq!(project_names(&h), ["one", "two", "app"]);
        let mut h = Harness::new(&[]);
        let default = h.core.workspace().unwrap();
        assert_eq!(owned(&h, Some(default))[0].1, "*scratch*<main>");
        h.keys("C-g");
        assert_eq!(owned(&h, None)[0].1, "*Messages*");
    }

    #[test]
    fn a_new_workspace_is_current_with_its_own_scratch() {
        let mut h = Harness::new(&[]);
        let w = h.new_workspace("w", "p");
        assert!(h.core.current().unkillable);
        assert_eq!(h.core.workspace().unwrap(), w);
        assert_eq!(h.core.project_name(h.core.workspaces()[1].project), "p");
        assert_eq!((name(&h), h.text()), ("*scratch*<w>", String::new()));
        assert_eq!(h.core.current().workspace, Some(w));
    }

    #[test]
    fn a_file_open_in_another_workspace_gets_a_second_buffer() {
        let mut h = Harness::new(&[("a.txt", "abc")]);
        h.open("a.txt");
        let first = h.core.windows.window.buffer();
        h.open("a.txt");
        assert_eq!(
            h.core.windows.window.buffer(),
            first,
            "reused in one workspace"
        );
        let w = h.new_workspace("w", "p");
        h.open("a.txt");
        assert_ne!(h.core.windows.window.buffer(), first);
        assert_eq!(
            (name(&h), h.core.current().workspace),
            ("a.txt<2>", Some(w))
        );
    }

    #[test]
    fn killing_a_buffer_never_shows_another_workspace_s_buffer() {
        let mut h = Harness::new(&[("a.txt", "abc"), ("b.txt", "def")]);
        h.open("a.txt");
        let w = h.new_workspace("w", "p");
        // a.txt is more recent than w's scratch.
        show(&mut h, "a.txt");
        h.open("b.txt").keys("C-x k RET");
        assert_eq!(
            (name(&h), h.core.current().workspace),
            ("*scratch*<w>", Some(w))
        );
    }

    #[test]
    fn killing_a_workspace_kills_its_buffers_and_nothing_else() {
        let mut h = Harness::new(&[("a.txt", "abc"), ("b.txt", "def")]);
        let default = h.core.workspace().unwrap();
        h.open("a.txt").keys("x");
        let a = h.new_workspace("a", "p");
        h.open("a.txt").open("b.txt").keys("y C-g");
        let b = h.new_workspace("b", "q");
        h.open("b.txt");
        let (kept, messages) = (owned(&h, Some(b)), owned(&h, None));
        let kept_default = owned(&h, Some(default));
        assert_eq!(owned(&h, Some(a)).len(), 3);
        h.core.kill_workspace(a).unwrap();
        let launch = h.core.workspaces()[0].name.clone();
        assert_eq!(names(&h), [launch.as_str(), "b"]);
        assert!(owned(&h, Some(a)).is_empty(), "a's buffers are gone");
        assert_eq!(
            h.core.workspace().unwrap(),
            b,
            "another workspace stays current"
        );
        assert_eq!(owned(&h, Some(b)), kept);
        assert_eq!(owned(&h, Some(default)), kept_default);
        assert_eq!(owned(&h, None), messages);
        assert_eq!(h.text(), "def", "b's buffer still shown");
        show(&mut h, "a.txt");
        assert_eq!(h.text(), "xabc", "the other workspace's edit is untouched");
    }

    #[test]
    fn killing_the_current_workspace_selects_its_neighbour() {
        let mut h = Harness::new(&[]);
        let default = h.core.workspace().unwrap();
        let a = h.new_workspace("a", "p");
        let b = h.new_workspace("b", "p");
        h.core.kill_workspace(b).unwrap();
        assert_eq!(
            (h.core.workspace().unwrap(), h.core.current().workspace),
            (a, Some(a))
        );
        let c = h.new_workspace("c", "p");
        h.core.kill_workspace(a).unwrap();
        assert_eq!(
            h.core.workspace().unwrap(),
            c,
            "killing another keeps the current one"
        );
        h.core.kill_workspace(c).unwrap();
        assert_eq!(
            h.core.workspace().unwrap(),
            default,
            "the one before when last"
        );
        assert_eq!(name(&h), "*scratch*<main>");
    }

    #[test]
    fn killing_the_sole_workspace_shows_the_lobby_and_a_new_one_leaves_it() {
        let mut h = Harness::new(&[]);
        h.core.log("before");
        h.keys("C-x 2");
        let sole = h.core.workspace().unwrap();
        h.core.kill_workspace(sole).unwrap();
        assert!(h.core.workspaces().is_empty() && h.core.workspace().is_none());
        assert_eq!(name(&h), "*Workspaces*");
        assert!(h.core.windows.others.is_empty(), "one window");
        assert_eq!(h.core.current().workspace, None);
        assert!(h.core.buffers.find("*scratch*<main>").is_none());
        h.core.log("in the lobby");
        // Made from the lobby, a workspace owns its buffers and the lobby list goes.
        let id = h.new_workspace("w", "p");
        assert_eq!((h.core.workspace(), name(&h)), (Some(id), "*scratch*<w>"));
        assert_eq!(h.core.current().workspace, Some(id));
        assert!(h.core.buffers.find("*Workspaces*").is_none());
        // `*Messages*` outlives both trips.
        h.core.kill_workspace(id).unwrap();
        let messages = h.core.buffers.find("*Messages*").unwrap();
        let messages = h.core.buffers.get(messages).unwrap();
        assert!(
            messages
                .text()
                .to_string()
                .starts_with("before\nin the lobby\n")
        );
    }

    /// Each shown window's buffer name and point in tree order, and the selected one's index.
    fn layout(h: &Harness) -> (Vec<(String, usize)>, usize) {
        let core = &h.core;
        let leaves = core.windows.tree.leaves();
        let shown = leaves.iter().map(|id| {
            let window = core.windows.others.get(id).unwrap_or(&core.windows.window);
            let buffer = core.buffers.get(window.buffer()).unwrap();
            (buffer.name.clone(), window.point(&core.buffers))
        });
        let selected = leaves.iter().position(|&id| id == core.windows.selected);
        (shown.collect(), selected.unwrap())
    }

    fn one(name: &str) -> (Vec<(String, usize)>, usize) {
        (vec![(name.into(), 0)], 0)
    }

    #[test]
    fn killing_a_clean_sole_workspace_by_key_asks_nothing() {
        for keys in ["C-x t 0", "C-x t k RET"] {
            let mut h = Harness::new(&[]);
            h.keys(keys);
            assert!(h.core.minibuffer().is_none(), "{keys}");
            assert!(h.core.workspaces().is_empty(), "{keys}");
            assert_eq!(name(&h), "*Workspaces*");
        }
    }

    #[test]
    fn killing_the_lobby_list_shows_messages() {
        let mut h = Harness::new(&[]);
        let sole = h.core.workspace().unwrap();
        h.core.kill_workspace(sole).unwrap();
        h.keys("C-x k RET");
        assert_eq!(name(&h), "*Messages*");
        h.keys("C-x t l");
        assert_eq!(name(&h), "*Workspaces*");
    }

    #[test]
    fn switching_away_and_back_restores_splits_buffers_and_points() {
        let mut h = Harness::new(&[("a.txt", "abc"), ("b.txt", "def")]);
        let default = h.core.workspace().unwrap();
        h.open("a.txt").keys("C-f C-f C-x 2 C-x o");
        h.open("b.txt").keys("C-f");
        let before = layout(&h);
        assert_eq!(before, (vec![("a.txt".into(), 2), ("b.txt".into(), 1)], 1));
        let w = h.new_workspace("w", "p");
        assert_eq!(
            layout(&h),
            one("*scratch*<w>"),
            "a new workspace has one window"
        );
        h.core.switch_workspace(default).unwrap();
        assert_eq!(layout(&h), before);
        assert_eq!(name(&h), "b.txt", "the selected window's buffer is current");
        h.core.switch_workspace(w).unwrap();
        assert_eq!(layout(&h), one("*scratch*<w>"));
    }

    #[test]
    fn switching_preserves_marks_rings_and_scroll() {
        let mut h = Harness::new(&[("a", "zero\none\ntwo\nthree\nfour\n")]);
        h.open("a").keys("C-x 2 C-x o");
        let origin = h.core.workspace().unwrap();
        let selected = h.core.windows.selected;
        let window = &mut h.core.windows.window;
        window.push_mark(&mut h.core.buffers, 1, true).unwrap();
        window.push_mark(&mut h.core.buffers, 4, true).unwrap();
        window.set_point(&mut h.core.buffers, 9).unwrap();
        window.set_top(1);
        assert_eq!(window.scroll(&h.core.buffers, 3), 1);
        let left = window.hscroll(20, 5);
        let other = h.new_workspace("other", "home");
        assert_eq!(h.core.workspace().unwrap(), other);
        assert_eq!(name(&h), "*scratch*<other>");
        h.core.switch_workspace(origin).unwrap();
        assert_eq!(h.core.windows.selected, selected);
        assert_eq!(h.core.windows.tree.leaves().len(), 2);
        let window = &mut h.core.windows.window;
        assert_eq!(window.point(&h.core.buffers), 9);
        assert_eq!(window.mark(&h.core.buffers), Some(4));
        assert!(window.active());
        assert_eq!(window.scroll(&h.core.buffers, 3), 1);
        assert_eq!(window.left(), left);
        window.pop_mark();
        assert_eq!(window.mark(&h.core.buffers), Some(1));
    }

    #[test]
    fn window_changes_stay_in_their_workspace() {
        let mut h = Harness::new(&[]);
        let default = h.core.workspace().unwrap();
        h.keys("C-x 2");
        let w = h.new_workspace("w", "p");
        h.keys("C-x 3 C-x 3");
        h.core.switch_workspace(default).unwrap();
        assert_eq!(layout(&h).0.len(), 2);
        h.keys("C-x 1");
        h.core.switch_workspace(w).unwrap();
        assert_eq!(layout(&h).0.len(), 3, "w keeps its three windows");
        h.core.switch_workspace(default).unwrap();
        assert_eq!(layout(&h), one("*scratch*<main>"));
    }

    #[test]
    fn points_follow_edits_made_while_away() {
        let mut h = Harness::new(&[("a.txt", "abc")]);
        let default = h.core.workspace().unwrap();
        h.open("a.txt").keys("C-e");
        h.new_workspace("w", "p");
        show(&mut h, "a.txt");
        h.keys("C-a X Y");
        h.core.switch_workspace(default).unwrap();
        assert_eq!((h.text(), h.point()), ("XYabc".into(), 5));
    }

    #[test]
    fn switching_closes_the_minibuffer_and_to_the_current_one_does_nothing() {
        let mut h = Harness::new(&[]);
        let default = h.core.workspace().unwrap();
        h.keys("C-x 2");
        let w = h.new_workspace("w", "p");
        ask(&mut h, &["Text: "]);
        h.keys("C-c p a b");
        h.core.switch_workspace(default).unwrap();
        assert!(h.core.minibuffer().is_none());
        assert_eq!(layout(&h), (vec![("*scratch*<main>".into(), 0); 2], 0));
        h.core.switch_workspace(default).unwrap();
        assert_eq!(layout(&h).0.len(), 2);
        h.core.switch_workspace(w).unwrap();
        assert_eq!(
            layout(&h),
            one("*scratch*<w>"),
            "no minibuffer window saved"
        );
    }

    #[test]
    fn killing_the_current_workspace_shows_the_neighbour_s_windows() {
        let mut h = Harness::new(&[]);
        h.keys("C-x 3");
        let w = h.new_workspace("w", "p");
        h.keys("C-x 2");
        h.core.kill_workspace(w).unwrap();
        assert_eq!(layout(&h), (vec![("*scratch*<main>".into(), 0); 2], 0));
        let frame = h.frame(800., 600.);
        assert!(
            frame.windows[0].rect.x < frame.windows[1].rect.x,
            "side by side"
        );
    }

    #[test]
    fn a_killed_buffer_is_replaced_in_a_workspace_that_is_not_current() {
        let mut h = Harness::new(&[("a.txt", "abc"), ("b.txt", "def")]);
        let default = h.core.workspace().unwrap();
        let w = h.new_workspace("w", "p");
        h.open("a.txt");
        h.core.switch_workspace(default).unwrap();
        show(&mut h, "a.txt");
        h.core.switch_workspace(w).unwrap();
        h.keys("C-x k RET");
        h.core.switch_workspace(default).unwrap();
        assert_eq!(layout(&h), one("*scratch*<main>"));
        // The same when its workspace is killed while a third one is current.
        h.open("b.txt").keys("C-x 2");
        h.core.switch_workspace(w).unwrap();
        h.open("b.txt");
        let b = h.core.windows.window.buffer();
        h.core.switch_workspace(default).unwrap();
        show(&mut h, "b.txt<2>");
        h.new_workspace("x", "p");
        h.core.kill_workspace(w).unwrap();
        assert!(h.core.buffers.get(b).is_none());
        h.core.switch_workspace(default).unwrap();
        assert_eq!(layout(&h).0[0].0, "b.txt");
    }

    /// A harness with a saved project `app` at `app/` with the additional root `lib/`, and
    /// ad-hoc projects `one` and `two`.
    fn projects() -> Harness {
        load_projects(Harness::new(FILES))
    }

    /// `projects()` in the lobby.
    fn lobby_projects() -> Harness {
        load_projects(Harness::lobby(FILES))
    }

    const FILES: &[(&str, &str)] = &[("app/x", ""), ("lib/x", ""), ("new/x", ""), ("f.txt", "")];

    fn load_projects(mut h: Harness) -> Harness {
        let (app, lib) = (h.path("app"), h.path("lib"));
        let text = format!(
            "name = \"app\"\n[primary]\npath = \"{}\"\n[[roots]]\npath = \"{}\"",
            app.display(),
            lib.display()
        );
        let saved = Loaded {
            project: Project::parse(&text).unwrap(),
            file: Some((h.path("projects/app.toml"), text)),
        };
        let one = Loaded::ad_hoc("one", &h.path("one"));
        let two = Loaded::ad_hoc("two", &h.path("two"));
        let dir = Some(h.path("projects"));
        let projects = vec![Ok(saved), Ok(one), Ok(two)];
        h.send(Msg::ProjectsLoaded { dir, projects });
        h
    }

    /// The current workspace's name and project.
    fn current(h: &Harness) -> (&str, &str) {
        let id = h.core.workspace().unwrap();
        let w = h.core.workspaces().iter().find(|w| w.id == id).unwrap();
        (w.name.as_str(), h.core.project_name(w.project))
    }

    fn project_names(h: &Harness) -> Vec<&str> {
        h.core.projects().iter().map(|p| p.name.as_str()).collect()
    }

    #[test]
    fn a_file_launches_in_the_saved_project_that_contains_it() {
        let mut h = lobby_projects();
        let file = h.path("lib/x");
        h.core.launch(&file, &h.path("lib"));
        assert_eq!((names(&h), current(&h)), (vec!["app"], ("app", "app")));
        assert_eq!(project_names(&h), ["one", "two", "app"], "no $HOME project");
    }

    #[test]
    fn a_file_outside_saved_projects_gets_an_ad_hoc_project_at_its_root() {
        let mut h = lobby_projects();
        let root = h.path("repo");
        h.core.launch(&root.join("src/x.rs"), &root);
        assert_eq!(current(&h), ("repo", "repo"));
        let repo = h.core.projects()[3];
        assert_eq!((&repo.name, &repo.primary.path), (&"repo".into(), &root));
        assert_eq!(
            h.core.projects[h.core.project(current(&h).1).unwrap()].file,
            None
        );
        // A taken name gets `<2>`, and an ad-hoc project at the root is reused.
        let mut h = lobby_projects();
        h.core.launch(&h.path("x/one/f"), &h.path("x/one"));
        assert_eq!(current(&h), ("one<2>", "one<2>"));
        let mut h = lobby_projects();
        h.core.launch(&h.path("one/f"), &h.path("one"));
        assert_eq!(current(&h).1, "one");
        assert_eq!(project_names(&h), ["one", "two", "app"]);
    }

    #[test]
    fn a_launch_at_home_is_named_after_it() {
        let mut h = Harness::lobby(&[]);
        let home = std::env::home_dir().unwrap();
        h.core.launch(&home.join("notes.txt"), &home);
        let name = crate::buffer::file_name(&home);
        assert_eq!(current(&h), (name.as_str(), name.as_str()));
        assert_eq!(h.core.projects()[0].primary.path, home);
    }

    #[test]
    fn new_workspace_prompts_for_a_project_and_is_named_after_it() {
        let mut h = projects();
        let launch = names(&h)[0].to_string();
        h.keys("C-x t 2 one RET RET");
        assert_eq!(h.core.minibuffer(), None);
        h.keys("C-x t 2 one RET RET");
        assert_eq!(names(&h), [launch.as_str(), "one", "one<2>"]);
        assert_eq!(
            (current(&h), name(&h)),
            (("one<2>", "one"), "*scratch*<one<2>>")
        );
        h.keys("C-x t 2 one RET mine RET");
        assert_eq!(
            (current(&h), name(&h)),
            (("mine", "one"), "*scratch*<mine>")
        );
        h.keys("C-x t 2 two RET mine RET");
        assert_eq!(h.core.message(), Some("Workspace mine already exists"));
        assert_eq!(names(&h).len(), 4);
        h.keys("C-x t 2 zz M-RET");
        assert_eq!(h.core.message(), Some("No project named zz"));
        assert_eq!(names(&h).len(), 4);
    }

    #[test]
    fn new_workspace_defaults_to_the_active_project_but_can_select_another() {
        let mut h = projects();
        h.keys("C-x t 2 two RET existing RET C-x t 2");
        assert_eq!(h.core.prompts.selected_candidate().as_deref(), Some("two"));
        h.keys("RET task RET");
        assert_eq!(current(&h), ("task", "two"));
        h.keys("C-x t 2 one RET RET");
        assert_eq!(current(&h), ("one", "one"));
        h.keys("C-x t 2 RET RET");
        assert_eq!(current(&h), ("one<2>", "one"));
    }

    #[test]
    fn new_workspace_accepts_the_selected_project_identity_even_with_equal_names() {
        let mut h = Harness::new(&[]);
        let first = h.core.add_project(Loaded::ad_hoc("app", &h.path("first")));
        let second = h.core.add_project(Loaded::ad_hoc("app", &h.path("second")));
        h.core.new_workspace("second", second);
        h.keys("C-x t 2 RET task RET");
        assert_eq!(h.core.current_workspace().unwrap().project, second);
        h.keys("C-x t 2 app C-n RET other RET");
        assert_eq!(
            h.core.current_workspace().unwrap().project,
            second,
            "filtered matches retain identity"
        );
        h.keys("C-x t 2 app RET first RET");
        assert_eq!(h.core.current_workspace().unwrap().project, first);
    }

    #[test]
    fn new_workspace_cancellation_keeps_the_workspace_and_layout() {
        let mut h = projects();
        h.keys("text C-x 2");
        let workspace = h.core.workspace().unwrap();
        let selected = h.core.windows.selected;
        for keys in ["C-x t 2 C-g", "C-x t 2 RET C-g"] {
            h.keys(keys);
            assert_eq!(h.core.workspace().unwrap(), workspace);
            assert_eq!(h.core.windows.selected, selected);
            assert_eq!(h.core.windows.tree.leaves().len(), 2);
            assert_eq!(h.text(), "text");
        }
    }

    #[test]
    fn new_workspace_keeps_the_selected_identity_across_reload_or_reports_its_removal() {
        for remove in [false, true] {
            let mut h = projects();
            let selected = h.core.project_id("app").unwrap();
            h.keys("C-x t 2 app RET");
            let count = h.core.workspaces.len();
            let projects = if remove {
                vec![]
            } else {
                let mut loaded = Loaded::ad_hoc("renamed", &h.path("app"));
                loaded.file = Some((h.path("projects/app.toml"), String::new()));
                vec![Ok(loaded)]
            };
            h.send(Msg::ProjectsLoaded {
                dir: Some(h.path("projects")),
                projects,
            });
            h.keys("task RET");
            if remove {
                assert_eq!(h.core.workspaces.len(), count);
                assert_eq!(h.core.message(), Some("Project is no longer loaded"));
            } else {
                assert_eq!(h.core.current_workspace().unwrap().project, selected);
                assert_eq!(current(&h), ("task", "renamed"));
            }
        }
    }

    #[test]
    fn switch_lists_the_others_most_recent_first_and_ret_goes_back() {
        let mut h = projects();
        let launch = names(&h)[0].to_string();
        h.keys("C-x t 2 one RET RET C-x t 2 two RET RET C-x t RET");
        assert_eq!(listed(&mut h).0, ["one", launch.as_str(), "two"]);
        h.keys("RET");
        assert_eq!(current(&h).0, "one");
        h.keys("C-x t RET zz M-RET");
        assert_eq!(h.core.message(), Some("No workspace named zz"));
        h.keys("M-x workspace-switch RET two RET");
        assert_eq!(current(&h).0, "two");
    }

    #[test]
    fn next_and_previous_wrap_and_take_a_count() {
        let mut h = projects();
        let launch = names(&h)[0].to_string();
        h.keys("C-x t 2 one RET RET C-x t 2 two RET RET C-x t o");
        assert_eq!(current(&h).0, launch);
        h.keys("C-x t O");
        assert_eq!(current(&h).0, "two");
        h.keys("C-x t o C-u 2 C-x t o");
        assert_eq!(current(&h).0, "two");
        h.keys("C-u 2 C-x t O");
        assert_eq!(current(&h).0, launch);
        h.keys("C-u - 1 C-x t o");
        assert_eq!(current(&h).0, "two", "negative goes back");
    }

    #[test]
    fn rename_refuses_empty_and_taken_names() {
        let mut h = projects();
        h.keys("C-x t 2 one RET RET C-x t r work RET");
        assert_eq!(current(&h), ("work", "one"));
        h.keys("C-x t 2 two RET RET C-x t r work RET");
        assert_eq!(h.core.message(), Some("Workspace work already exists"));
        h.keys("C-x t r RET");
        assert_eq!(h.core.message(), Some("Empty workspace name"));
        assert_eq!(current(&h).0, "two");
    }

    #[test]
    fn list_shows_names_and_projects_with_the_current_one_marked() {
        let mut h = projects();
        h.keys("C-x t 2 one RET RET C-x t r a RET C-x t 2 two RET RET C-x t r b RET");
        h.core.kill_workspace(h.core.workspaces()[0].id).unwrap();
        h.keys("C-x t l");
        let lines: Vec<_> = h
            .text()
            .lines()
            .map(|s| s.split_whitespace().collect::<Vec<_>>().join(" "))
            .collect();
        assert_eq!(
            lines,
            [
                "▾ one /aiditor-test/one".into(),
                "a 0 buffers".into(),
                "".into(),
                "▾ two /aiditor-test/two".into(),
                "* b 0 buffers".into(),
                "".into(),
                "app /aiditor-test/app · no workspaces".into(),
                "".into(),
                format!(
                    "home {} · no workspaces",
                    h.core.projects[0].project.primary.path.display()
                ),
            ]
        );
    }

    /// The harness of `projects` with a second workspace `two`, current, visiting a modified
    /// `f.txt`.
    fn modified() -> Harness {
        let mut h = projects();
        h.keys("C-x t 2 two RET RET");
        h.open("f.txt").keys("x");
        h
    }

    #[test]
    fn killing_the_first_workspace_while_current_shows_the_next() {
        let mut h = projects();
        h.keys("C-x t 2 one RET RET C-x t o C-x t 0");
        assert_eq!((names(&h), current(&h).0), (vec!["one"], "one"));
    }

    #[test]
    fn kill_with_nothing_modified_kills_at_once() {
        let mut h = projects();
        let launch = names(&h)[0].to_string();
        h.keys("C-x t 2 one RET RET C-x t k");
        assert_eq!(listed(&mut h).0, ["one", launch.as_str()], "current first");
        h.keys("RET");
        assert_eq!(
            (names(&h), current(&h).0),
            (vec![launch.as_str()], launch.as_str())
        );
        assert_eq!(h.core.minibuffer(), None);
    }

    #[test]
    fn killing_a_workspace_confirms_its_live_terminal() {
        for command in ["C-x t 0", "C-x t k RET"] {
            let mut h = Harness::new(&[]);
            let id = h.new_workspace("b", "home");
            h.keys("M-x vterm RET");
            h.keys(command);
            assert_eq!(
                h.core.minibuffer().unwrap().prompt,
                "Active processes exist; kill them and the workspace anyway? "
            );
            assert_eq!(listed(&mut h).0, ["No", "Yes"]);
            h.keys("yes RET");
            assert_eq!(h.core.workspaces.len(), 1);
            assert!(h.core.workspaces.iter().all(|w| w.id != id));
            assert!(
                h.terminal_ops
                    .iter()
                    .any(|op| matches!(op, crate::TerminalOp::Kill { .. }))
            );
        }
    }

    #[test]
    fn declining_keeps_the_session_workspace_and_agent() {
        let mut h = Harness::new(&[]);
        let id = h.new_workspace("b", "home");
        h.keys("M-x agent-start RET claude RET");
        let buffer = h.core.windows.window.buffer();
        h.keys("C-x t 0 RET");
        assert_eq!(h.core.workspace().unwrap(), id);
        assert_eq!(h.core.windows.window.buffer(), buffer);
        assert!(h.core.live_agent(buffer));
    }

    #[test]
    fn declining_process_kill_still_saves_the_yes_buffers() {
        let mut h = modified();
        let id = h.core.workspace().unwrap();
        h.keys("M-x vterm RET C-x t 0 RET");
        assert_eq!(
            h.core.minibuffer().unwrap().prompt,
            "Active processes exist; kill them and the workspace anyway? "
        );
        h.save_reply("x\n");
        h.keys("RET");
        assert_eq!(h.submitted("f.txt"), "x\n");
        assert_eq!(h.core.workspace().unwrap(), id);
    }

    #[test]
    fn other_workspaces_processes_and_ended_sessions_need_no_question() {
        let mut h = Harness::new(&[]);
        h.keys("M-x vterm RET");
        h.new_workspace("b", "home");
        h.keys("C-x t 0");
        assert_eq!(h.core.workspaces.len(), 1);
        h.new_workspace("b", "home");
        h.keys("M-x agent-start RET claude RET");
        h.send(Msg::AgentExited {
            id: crate::AgentId(0),
            code: 0,
            signal: None,
        });
        h.keys("C-x t 0");
        assert_eq!(h.core.workspaces.len(), 1);
    }

    #[test]
    fn kill_saves_then_kills() {
        let mut h = modified();
        h.keys("C-x t 0");
        let prompt = format!("Save file {}? ", h.path("f.txt").display());
        assert_eq!(h.core.minibuffer().unwrap().prompt, prompt);
        h.save_reply("x\n");
        h.keys("RET");
        assert_eq!(h.submitted("f.txt"), "x\n");
        assert_eq!(names(&h).len(), 1, "killed once saved");
    }

    #[test]
    fn declining_keeps_the_workspace_unless_killed_anyway() {
        let mut h = modified();
        h.keys("C-x t 0 no RET");
        let prompt = "Modified buffers exist; kill anyway? ";
        assert_eq!(h.core.minibuffer().unwrap().prompt, prompt);
        h.keys("RET");
        assert_eq!((names(&h).len(), h.text()), (2, "x".into()));
        h.keys("C-x t k RET no RET yes RET");
        assert_eq!(names(&h).len(), 1);
        assert_eq!(h.writes, Vec::<(std::path::PathBuf, String)>::new());
    }

    #[test]
    fn a_failed_save_keeps_the_workspace() {
        let mut h = modified();
        h.read_reply("f.txt", Some(b"theirs"));
        h.saves.push_back(Err(format!(
            "{} changed on disk; not saved",
            h.path("f.txt").display()
        )));
        h.keys("C-x t 0 RET");
        let err = format!("{} changed on disk; not saved", h.path("f.txt").display());
        assert_eq!(h.core.message(), Some(err.as_str()));
        assert_eq!(names(&h).len(), 2);
        // The cancelled kill doesn't come back with a later save.
        h.save_reply("x\n");
        h.keys("C-x C-s Keep SPC mine RET");
        assert_eq!(h.submitted("f.txt"), "x\n");
        assert_eq!(names(&h).len(), 2);
    }

    #[test]
    fn kill_asks_only_about_its_own_buffers() {
        let mut h = modified();
        h.keys("C-x t 2 one RET RET C-x t 0");
        assert_eq!(h.core.minibuffer(), None);
        assert_eq!(names(&h).len(), 2, "one killed without a question");
    }

    #[test]
    fn the_sole_workspace_is_asked_about_then_killed() {
        let mut h = Harness::new(&[("f.txt", "")]);
        h.open("f.txt").keys("x C-x t 0 no RET RET");
        assert_eq!(h.core.workspaces().len(), 1, "declining keeps it");
        assert_eq!(name(&h), "f.txt");
        h.keys("C-x t k RET no RET yes RET");
        assert!(h.core.workspaces().is_empty());
        assert_eq!(name(&h), "*Workspaces*");
    }

    #[test]
    fn a_folder_launches_in_an_ad_hoc_project_that_a_reload_keeps() {
        let mut h = lobby_projects();
        let new = h.path("new");
        h.core.launch(&new, &new);
        assert_eq!((current(&h), name(&h)), (("new", "new"), "*scratch*<new>"));
        let project = (h.core.projects().into_iter()).find(|p| p.name == "new");
        let project = project.unwrap().clone();
        assert_eq!(project.primary.path, new);
        assert_eq!(
            h.core.projects[h.core.project(current(&h).1).unwrap()].file,
            None
        );
        h.send(Msg::ProjectsLoaded {
            dir: Some(h.path("projects")),
            projects: Vec::new(),
        });
        assert!(h.core.projects().contains(&&project));
    }

    #[test]
    fn a_folder_inside_a_saved_project_launches_in_it() {
        let mut h = lobby_projects();
        h.core.launch(&h.path("lib/src"), &h.path("lib/src"));
        assert_eq!(current(&h), ("app", "app"));
        assert_eq!(project_names(&h), ["one", "two", "app"]);
    }

    #[test]
    fn switch_project_shows_its_most_recent_workspace_or_makes_one() {
        let mut h = projects();
        h.keys("C-x t 2 one RET RET C-x t 2 one RET RET C-x t 2 two RET RET");
        let latest = h.core.workspaces()[2].id;
        h.keys("C-x p p one RET");
        assert_eq!(h.core.workspace().unwrap(), latest);
        h.keys("C-x t RET one RET C-x t RET two RET C-x p p one RET");
        assert_eq!(current(&h).0, "one", "the one shown last");
        h.keys("C-x p p app RET");
        assert_eq!((names(&h).len(), current(&h)), (5, ("app", "app")));
    }

    #[test]
    fn c_x_t_lists_the_workspace_commands() {
        let mut h = Harness::new(&[]);
        h.keys("C-x t").advance(crate::bindings::HINT_DELAY);
        let hint = format!("{:?}", h.core.hint());
        for command in [
            "workspace-new",
            "workspace-switch",
            "workspace-next",
            "workspace-previous",
            "workspace-rename",
            "workspace-kill",
            "workspace-list",
            "workspace-kill-current",
        ] {
            assert!(hint.contains(command), "{command} in {hint}");
        }
    }

    fn visit(h: &mut Harness, file: &str) {
        let path = h.path(file).to_string_lossy().into_owned();
        h.call(
            "find-file",
            crate::command::Args {
                strings: vec![path],
                ..crate::command::Args::default()
            },
        );
    }

    #[test]
    fn find_file_from_the_lobby_makes_the_file_s_workspace() {
        let mut h = lobby_projects();
        assert_eq!(
            h.core.directory(),
            std::env::home_dir().unwrap(),
            "starts at home"
        );
        visit(&mut h, "app/x");
        assert_eq!((current(&h), name(&h)), (("app", "app"), "x"));
        assert_eq!(h.core.current().workspace, h.core.workspace());
        assert!(h.core.buffers.find("*Workspaces*").is_none());
        // Outside every project, the file's folder, the test dir holding no `.git`.
        let mut h = lobby_projects();
        visit(&mut h, "f.txt");
        assert_eq!(current(&h), ("aiditor-test", "aiditor-test"));
        let project = h
            .core
            .projects()
            .into_iter()
            .find(|p| p.name == "aiditor-test");
        assert_eq!(project.unwrap().primary.path, h.path(""));
    }

    #[test]
    fn a_failed_visit_from_the_lobby_stays_in_it() {
        let mut h = lobby_projects();
        h.reads.insert(h.path("f.txt"), Err("f.txt: denied".into()));
        visit(&mut h, "f.txt");
        assert_eq!(h.core.message(), Some("f.txt: denied"));
        assert_eq!((h.core.workspace(), name(&h)), (None, "*Workspaces*"));
        assert_eq!(project_names(&h), ["one", "two", "app"]);
    }

    #[test]
    fn workspace_new_and_project_switch_leave_the_lobby() {
        let mut h = lobby_projects();
        h.keys("C-x t 2 RET RET");
        assert_eq!(current(&h), ("one", "one"), "the first project is selected");
        assert!(h.core.buffers.find("*Workspaces*").is_none());
        let mut h = lobby_projects();
        h.keys("C-x 2 C-x p p app RET");
        assert_eq!((current(&h), name(&h)), (("app", "app"), "*scratch*<app>"));
        assert!(h.core.windows.others.is_empty(), "one window");
        assert!(h.core.buffers.find("*Workspaces*").is_none());
    }

    #[test]
    fn commands_that_need_a_workspace_fail_in_the_lobby() {
        for command in [
            "project-find-file",
            "project-grep",
            "vterm",
            "agent-start",
            "agent-sessions",
            "workspace-switch",
            "workspace-next",
            "workspace-previous",
            "workspace-rename",
            "workspace-kill",
            "workspace-kill-current",
            "getenv",
            "project-capture-environment",
        ] {
            let mut h = lobby_projects();
            h.call(command, crate::command::Args::default());
            assert_eq!(h.core.message(), Some("No workspace"), "{command}");
            assert!(h.core.minibuffer().is_none(), "{command} asks nothing");
            assert_eq!((h.core.workspace(), name(&h)), (None, "*Workspaces*"));
        }
        // `workspace-list` refreshes the lobby's.
        let mut h = lobby_projects();
        h.keys("C-x t l");
        assert_eq!((h.core.message(), name(&h)), (None, "*Workspaces*"));
    }
}
