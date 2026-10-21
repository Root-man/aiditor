//! Workspaces: everything for one task, bound to one project, owning its buffers and its
//! window configuration.

use std::cmp::Reverse;
use std::collections::HashMap;
use std::path::Path;

use crate::buffer::{scratch_name, unique};
use crate::command::Args;
use crate::window::{Tree, Window, WindowId};
use crate::{Core, Ctx};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct WorkspaceId(u64);

pub struct Workspace {
    pub id: WorkspaceId,
    pub name: String,
    /// The name of the project it is bound to.
    pub project: String,
    /// When it was last shown, counting shows; the highest is the most recent.
    pub(crate) shown: u64,
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
    pub(crate) fn new(id: WorkspaceId, name: &str, project: &str) -> Self {
        Workspace {
            id,
            name: name.into(),
            project: project.into(),
            shown: 0,
            windows: None,
        }
    }
}

impl Core {
    /// Makes a workspace bound to `project` current, named `name` or `name<2>`, ... when that is
    /// taken, showing its new `*scratch*<name>` in a single window; the workspace that was current
    /// keeps its windows.
    pub fn new_workspace(&mut self, name: &str, project: &str) -> WorkspaceId {
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
        self.shown += 1;
        let shown = self.shown;
        let new = (self.workspaces.iter_mut().find(|w| w.id == id)).expect("shown is live");
        new.shown = shown;
        self.buffers.workspace = id;
        self.buffers.raise(self.window.buffer());
    }

    /// Kills workspace `id`, its windows and every buffer it owns; killing the current one makes
    /// the one after it current, or the one before when it is last.
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

    /// Binds the launch workspace, in place of its `$HOME` project, to the project `path`
    /// belongs to, or else to an ad-hoc one at `root`, and names it after that project.
    pub fn launch(&mut self, path: &Path, root: &Path) {
        assert_eq!(self.workspaces.len(), 1, "only the launch workspace exists");
        let home = &self.workspaces[0].project;
        (self.projects).retain(|p| p.file.is_some() || p.project.name != *home);
        let project = self.project_for(path, root);
        self.workspaces[0].project = project.clone();
        (self.rename_workspace(&project)).expect("the sole workspace's new name is free");
    }

    pub(crate) fn current_workspace(&self) -> &Workspace {
        let current = self.workspace();
        (self.workspaces.iter().find(|w| w.id == current)).expect("current is live")
    }

    pub fn workspaces(&self) -> &[Workspace] {
        &self.workspaces
    }

    pub fn workspace(&self) -> WorkspaceId {
        self.buffers.workspace
    }

    /// The workspaces' names, most recently shown first, with the current one first or last.
    pub(crate) fn workspace_names(&self, current_first: bool) -> Vec<String> {
        let mut workspaces: Vec<&Workspace> = self.workspaces.iter().collect();
        let current = self.workspace();
        workspaces.sort_by_key(|w| ((w.id == current) != current_first, Reverse(w.shown)));
        workspaces.into_iter().map(|w| w.name.clone()).collect()
    }

    fn workspace_named(&self, name: &str) -> Result<WorkspaceId, String> {
        (self.workspaces.iter().find(|w| w.name == name))
            .map(|w| w.id)
            .ok_or_else(|| format!("No workspace named {name}"))
    }

    /// Makes a new workspace for the project `project`, named after it.
    pub(crate) fn workspace_for(&mut self, project: &str) -> Result<(), String> {
        self.project(project)?;
        self.new_workspace(project, project);
        Ok(())
    }

    pub(crate) fn switch_to_workspace(&mut self, name: &str) -> Result<(), String> {
        let id = self.workspace_named(name)?;
        self.switch_workspace(id)
    }

    /// Shows the workspace `n` after the current one in creation order, wrapping; before it
    /// when negative.
    pub(crate) fn cycle_workspace(&mut self, n: i64) -> Result<(), String> {
        let len = self.workspaces.len() as i64;
        let current = self.workspace();
        let at = (self.workspaces.iter().position(|w| w.id == current)).expect("current is live");
        let to = (at as i64 + n % len).rem_euclid(len) as usize;
        self.switch_workspace(self.workspaces[to].id)
    }

    pub(crate) fn rename_workspace(&mut self, name: &str) -> Result<(), String> {
        if name.trim().is_empty() {
            return Err("Empty workspace name".into());
        }
        let current = self.workspace();
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

    /// Shows every workspace and its project in the echo area, the current one marked `*`.
    pub(crate) fn list_workspaces(&mut self) {
        let current = self.workspace();
        let list: Vec<String> = (self.workspaces.iter())
            .map(|w| {
                let mark = if w.id == current { "*" } else { "" };
                format!("{mark}{} ({})", w.name, w.project)
            })
            .collect();
        self.echo(list.join("  "));
    }

    /// Shows the project `name`'s most recently shown workspace, or a new one when it has none.
    pub(crate) fn switch_project(&mut self, name: &str) -> Result<(), String> {
        let latest = (self.workspaces.iter())
            .filter(|w| w.project == name)
            .max_by_key(|w| w.shown);
        match latest.map(|w| w.id) {
            Some(id) => self.switch_workspace(id),
            None => self.workspace_for(name),
        }
    }
}

impl Ctx<'_> {
    /// Kills the workspace named by the first string once its modified file buffers are saved
    /// or confirmed, asking as `C-x C-c` does.
    pub(crate) fn kill_workspace(&mut self, args: Args) -> Result<(), String> {
        let id = self.core.workspace_named(&args.strings[0])?;
        if self.core.workspaces.len() == 1 {
            return Err("Attempt to delete the sole workspace".into());
        }
        let mine = |b: &crate::buffer::Buffer| b.workspace == Some(id);
        let Some(saving) = self.save_some("workspace-kill", args, 1, mine, "kill")? else {
            return Ok(());
        };
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

    fn names(h: &Harness) -> Vec<&str> {
        h.core
            .workspaces()
            .iter()
            .map(|w| w.name.as_str())
            .collect()
    }

    #[test]
    fn launch_has_one_workspace_on_home_and_messages_is_global() {
        let core = crate::Core::default();
        let (w, home) = (&core.workspaces()[0], &core.projects()[0]);
        assert_eq!(core.workspaces().len(), 1);
        assert_eq!((&w.name, &w.project), (&home.name, &home.name));
        assert_eq!(Some(home.primary.path.clone()), std::env::home_dir());
        assert_eq!(core.current().name, format!("*scratch*<{}>", home.name));
        let mut h = Harness::new(&[]);
        let default = h.core.workspace();
        assert_eq!(owned(&h, Some(default))[0].1, "*scratch*<main>");
        h.keys("C-g");
        assert_eq!(owned(&h, None)[0].1, "*Messages*");
    }

    #[test]
    fn a_new_workspace_is_current_with_its_own_scratch() {
        let mut h = Harness::new(&[]);
        let w = h.core.new_workspace("w", "p");
        assert!(h.core.current().unkillable);
        assert_eq!(h.core.workspace(), w);
        assert_eq!(h.core.workspaces()[1].project, "p");
        assert_eq!((name(&h), h.text()), ("*scratch*<w>", String::new()));
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
            ("*scratch*<w>", Some(w))
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
        let launch = h.core.workspaces()[0].name.clone();
        assert_eq!(names(&h), [launch.as_str(), "b"]);
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
        assert_eq!(name(&h), "*scratch*<main>");
    }

    #[test]
    fn the_sole_workspace_cannot_be_killed() {
        let mut h = Harness::new(&[]);
        let default = h.core.workspace();
        let err = h.core.kill_workspace(default);
        assert_eq!(err, Err("Attempt to delete the sole workspace".into()));
        assert_eq!(
            (h.core.workspaces().len(), name(&h)),
            (1, "*scratch*<main>")
        );
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
        assert_eq!(layout(&h), one("*scratch*<main>"));
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
        let w = h.core.new_workspace("w", "p");
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
        let default = h.core.workspace();
        let w = h.core.new_workspace("w", "p");
        h.open("a.txt");
        h.core.switch_workspace(default).unwrap();
        h.keys("C-x b a.txt RET");
        h.core.switch_workspace(w).unwrap();
        h.keys("C-x k RET");
        h.core.switch_workspace(default).unwrap();
        assert_eq!(layout(&h), one("*scratch*<main>"));
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

    /// A harness with a saved project `app` at `app/` with the additional root `lib/`, and
    /// ad-hoc projects `one` and `two`.
    fn projects() -> Harness {
        let mut h = Harness::new(&[("app/x", ""), ("lib/x", ""), ("new/x", ""), ("f.txt", "")]);
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
        let id = h.core.workspace();
        let w = h.core.workspaces().iter().find(|w| w.id == id).unwrap();
        (w.name.as_str(), w.project.as_str())
    }

    fn project_names(h: &Harness) -> Vec<&str> {
        h.core.projects().iter().map(|p| p.name.as_str()).collect()
    }

    #[test]
    fn a_file_launches_in_the_saved_project_that_contains_it() {
        let mut h = projects();
        let file = h.path("lib/x");
        h.core.launch(&file, &h.path("lib"));
        assert_eq!((names(&h), current(&h)), (vec!["app"], ("app", "app")));
        assert_eq!(project_names(&h), ["app", "one", "two"], "no $HOME project");
    }

    #[test]
    fn a_file_outside_saved_projects_gets_an_ad_hoc_project_at_its_root() {
        let mut h = projects();
        let root = h.path("repo");
        h.core.launch(&root.join("src/x.rs"), &root);
        assert_eq!(current(&h), ("repo", "repo"));
        let repo = h.core.projects()[3];
        assert_eq!((&repo.name, &repo.primary.path), (&"repo".into(), &root));
        assert!(!h.path("projects").exists(), "nothing written");
        // A taken name gets `<2>`, and an ad-hoc project at the root is reused.
        let mut h = projects();
        h.core.launch(&h.path("x/one/f"), &h.path("x/one"));
        assert_eq!(current(&h), ("one<2>", "one<2>"));
        let mut h = projects();
        h.core.launch(&h.path("one/f"), &h.path("one"));
        assert_eq!(current(&h).1, "one");
        assert_eq!(project_names(&h), ["app", "one", "two"]);
    }

    #[test]
    fn a_launch_at_home_keeps_the_home_name() {
        let mut h = Harness::new(&[]);
        let home = h.core.projects()[0].clone();
        h.core
            .launch(&home.primary.path.join("notes.txt"), &home.primary.path);
        assert_eq!(h.core.projects(), [&home], "the same project, not a `<2>`");
        assert_eq!(current(&h), (home.name.as_str(), home.name.as_str()));
    }

    #[test]
    fn new_workspace_prompts_for_a_project_and_is_named_after_it() {
        let mut h = projects();
        let launch = names(&h)[0].to_string();
        h.keys("C-x t 2 one RET");
        assert_eq!(h.core.minibuffer(), None);
        h.keys("C-x t 2 one RET");
        assert_eq!(names(&h), [launch.as_str(), "one", "one<2>"]);
        assert_eq!(
            (current(&h), name(&h)),
            (("one<2>", "one"), "*scratch*<one<2>>")
        );
        h.keys("C-x t 2 zz M-RET");
        assert_eq!(h.core.message(), Some("No project named zz"));
        assert_eq!(names(&h).len(), 3);
    }

    #[test]
    fn switch_lists_the_others_most_recent_first_and_ret_goes_back() {
        let mut h = projects();
        let launch = names(&h)[0].to_string();
        h.keys("C-x t 2 one RET C-x t 2 two RET C-x t RET");
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
        h.keys("C-x t 2 one RET C-x t 2 two RET C-x t o");
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
        h.keys("C-x t 2 one RET C-x t r work RET");
        assert_eq!(current(&h), ("work", "one"));
        h.keys("C-x t 2 two RET C-x t r work RET");
        assert_eq!(h.core.message(), Some("Workspace work already exists"));
        h.keys("C-x t r RET");
        assert_eq!(h.core.message(), Some("Empty workspace name"));
        assert_eq!(current(&h).0, "two");
    }

    #[test]
    fn list_shows_names_and_projects_with_the_current_one_marked() {
        let mut h = projects();
        h.keys("C-x t 2 one RET C-x t r a RET C-x t 2 two RET C-x t r b RET");
        h.core.kill_workspace(h.core.workspaces()[0].id).unwrap();
        h.keys("C-x t l");
        assert_eq!(h.core.message(), Some("a (one)  *b (two)"));
    }

    /// The harness of `projects` with a second workspace `two`, current, visiting a modified
    /// `f.txt`.
    fn modified() -> Harness {
        let mut h = projects();
        h.keys("C-x t 2 two RET");
        h.open("f.txt").keys("x");
        h
    }

    #[test]
    fn kill_with_nothing_modified_kills_at_once() {
        let mut h = projects();
        let launch = names(&h)[0].to_string();
        h.keys("C-x t 2 one RET C-x t k");
        assert_eq!(listed(&mut h).0, ["one", launch.as_str()], "current first");
        h.keys("RET");
        assert_eq!(
            (names(&h), current(&h).0),
            (vec![launch.as_str()], launch.as_str())
        );
        assert_eq!(h.core.minibuffer(), None);
    }

    #[test]
    fn kill_saves_then_kills() {
        let mut h = modified();
        h.keys("C-x t 0");
        let prompt = format!("Save file {}? ", h.path("f.txt").display());
        assert_eq!(h.core.minibuffer().unwrap().prompt, prompt);
        h.keys("RET");
        assert_eq!(h.disk("f.txt"), "x\n");
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
        assert_eq!(h.disk("f.txt"), "", "unchanged on disk");
    }

    #[test]
    fn a_failed_save_keeps_the_workspace() {
        let mut h = modified();
        std::fs::write(h.path("f.txt"), "theirs").unwrap();
        h.keys("C-x t 0 RET");
        let err = format!("{} changed on disk; not saved", h.path("f.txt").display());
        assert_eq!(h.core.message(), Some(err.as_str()));
        assert_eq!(names(&h).len(), 2);
        // The cancelled kill doesn't come back with a later save.
        std::fs::write(h.path("f.txt"), "").unwrap();
        h.keys("C-x C-s");
        assert_eq!(h.disk("f.txt"), "x\n");
        assert_eq!(names(&h).len(), 2);
    }

    #[test]
    fn kill_asks_only_about_its_own_buffers() {
        let mut h = modified();
        h.keys("C-x t 2 one RET C-x t 0");
        assert_eq!(h.core.minibuffer(), None);
        assert_eq!(names(&h).len(), 2, "one killed without a question");
    }

    #[test]
    fn the_sole_workspace_is_refused_before_any_question() {
        let mut h = Harness::new(&[("f.txt", "")]);
        let sole = Some("Attempt to delete the sole workspace");
        h.open("f.txt").keys("x C-x t 0");
        assert!(h.core.minibuffer().is_none());
        assert_eq!(h.core.message(), sole);
        h.keys("C-x t k RET");
        assert!(h.core.minibuffer().is_none());
        assert_eq!(h.core.message(), sole);
    }

    #[test]
    fn a_folder_launches_in_an_ad_hoc_project_that_a_reload_keeps() {
        let mut h = projects();
        let new = h.path("new");
        h.core.launch(&new, &new);
        assert_eq!((current(&h), name(&h)), (("new", "new"), "*scratch*<new>"));
        let project = (h.core.projects().into_iter()).find(|p| p.name == "new");
        let project = project.unwrap().clone();
        assert_eq!(project.primary.path, new);
        assert!(!h.path("projects").exists(), "nothing written");
        h.send(Msg::ProjectsLoaded {
            dir: Some(h.path("projects")),
            projects: Vec::new(),
        });
        assert!(h.core.projects().contains(&&project));
    }

    #[test]
    fn a_folder_inside_a_saved_project_launches_in_it() {
        let mut h = projects();
        h.core.launch(&h.path("lib/src"), &h.path("lib/src"));
        assert_eq!(current(&h), ("app", "app"));
        assert_eq!(project_names(&h), ["app", "one", "two"]);
    }

    #[test]
    fn switch_project_shows_its_most_recent_workspace_or_makes_one() {
        let mut h = projects();
        h.keys("C-x t 2 one RET C-x t 2 one RET C-x t 2 two RET");
        let latest = h.core.workspaces()[2].id;
        h.keys("C-x p p one RET");
        assert_eq!(h.core.workspace(), latest);
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
}
