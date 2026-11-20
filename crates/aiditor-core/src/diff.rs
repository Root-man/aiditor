//! The diff view (M9): a temporary full-frame layout of the current workspace, the changed-file
//! list on the left and a window for the diff on the right.

use std::path::{Path, PathBuf};

use crate::buffer::{BufferId, FaceRef, Kind, Prop};
use crate::settings::FaceName;
use crate::window::{Tree, Window, WindowId};
use crate::workspace::Windows;
use crate::{Core, Ctx, Effect, FileOp, GitChange, GitStatus};

pub(crate) const MODE: &str = "diff-list-mode";
/// The list's share of the frame: frame 15's 460 of 1440 px.
const RATIO: f32 = 0.32;

/// The echo line's actions in the list.
pub(crate) const HINTS: &[(&[&str], &str)] =
    &[(&["diff-refresh"], "refresh"), (&["diff-quit"], "quit")];

/// A line of the list point can be kept on across a refresh.
#[derive(Clone, Debug, PartialEq)]
enum Row {
    Root(PathBuf),
    File {
        root: PathBuf,
        staged: bool,
        path: PathBuf,
    },
}

type Answer = Vec<(PathBuf, Result<Option<GitStatus>, String>)>;

/// A changed-file list's state.
pub(crate) struct DiffList {
    roots: Vec<PathBuf>,
    /// The latest status shown; `None` until the first arrives.
    answer: Option<Answer>,
    /// Statuses asked and not yet answered.
    pending: usize,
    rows: Vec<(usize, Row)>,
}

/// The status letter of a change and its face.
fn letter(change: GitChange) -> (&'static str, FaceName) {
    match change {
        GitChange::Modified => ("M", FaceName::DiffFileModified),
        GitChange::TypeChanged => ("T", FaceName::DiffFileModified),
        GitChange::Renamed => ("R", FaceName::DiffFileModified),
        GitChange::Added => ("A", FaceName::DiffFileAdded),
        GitChange::Copied => ("C", FaceName::DiffFileAdded),
        GitChange::Deleted => ("D", FaceName::DiffFileDeleted),
        GitChange::Unmerged => ("U", FaceName::DiffFileDeleted),
        GitChange::Untracked => ("?", FaceName::DiffFileUntracked),
    }
}

impl Core {
    /// The current workspace's changed-file list.
    fn diff_list_here(&self) -> Option<BufferId> {
        let here = self.workspace()?;
        (self.buffers.recent())
            .find(|(_, b)| b.diff_list().is_some() && b.workspace == Some(here))
            .map(|(id, _)| id)
    }

    /// Takes a status answer into every list waiting for these roots.
    pub(crate) fn on_git_status(&mut self, answer: Answer) {
        let roots: Vec<_> = answer.iter().map(|(root, _)| root.clone()).collect();
        let waiting: Vec<_> = (self.buffers.iter_mut())
            .filter_map(|(id, b)| {
                let list = b.diff_list_mut()?;
                (list.pending > 0 && list.roots == roots).then(|| {
                    (list.pending, list.answer) = (list.pending - 1, Some(answer.clone()));
                    id
                })
            })
            .collect();
        for id in waiting {
            self.render_diff_list(id);
        }
    }

    /// Rebuilds list `id`'s text from its answer, keeping point on its row.
    fn render_diff_list(&mut self, id: BufferId) {
        let Some(buffer) = self.buffers.get(id) else {
            return;
        };
        let Some(list) = buffer.diff_list() else {
            return;
        };
        // Point where a window shows the buffer, else the buffer's own.
        let saved = self.workspaces.iter().filter_map(|w| w.windows.as_ref());
        let point = (std::iter::once(&self.windows).chain(saved))
            .flat_map(|w| std::iter::once(&w.window).chain(w.others.values()))
            .find(|w| w.buffer() == id)
            .map_or_else(
                || buffer.marker(buffer.point).unwrap_or(0),
                |w| w.point(&self.buffers),
            );
        let line = buffer.line_col(point).map_or(0, |(line, _)| line);
        let at = (list.rows.iter())
            .find(|(l, _)| *l == line)
            .map(|(_, r)| r.clone());
        let face = |face| Some(Prop::Named(vec![FaceRef::Face(face)]));
        let bold = || Some(Prop::Named(vec![FaceRef::Bold]));
        let mut lines: Vec<Vec<(String, Option<Prop>)>> = Vec::new();
        let mut rows = Vec::new();
        match &list.answer {
            None => lines.push(vec![("Reading status...".into(), None)]),
            Some(answer) => {
                for (root, result) in answer {
                    if !lines.is_empty() {
                        lines.push(Vec::new());
                    }
                    let name = (root.file_name()).map_or_else(
                        || root.display().to_string(),
                        |n| n.to_string_lossy().into_owned(),
                    );
                    rows.push((lines.len(), Row::Root(root.clone())));
                    // Two columns for the fold mark frame 15 draws there.
                    let mut heading = vec![("  ".into(), None), (name, bold())];
                    let status = match result {
                        Ok(Some(status)) => status,
                        Ok(None) => {
                            heading.push((
                                "  not a git repository".into(),
                                face(FaceName::Annotation),
                            ));
                            lines.push(heading);
                            continue;
                        }
                        // On one line, so each row's line number holds.
                        Err(err) => {
                            let err = err.split_whitespace().collect::<Vec<_>>().join(" ");
                            heading.push((format!("  {err}"), face(FaceName::StatusFailed)));
                            lines.push(heading);
                            continue;
                        }
                    };
                    heading.push(("  ".into(), None));
                    heading.push((format!("⎇ {}", status.branch), face(FaceName::VcsBranch)));
                    lines.push(heading);
                    if status.files.is_empty() {
                        lines.push(vec![("    No changes".into(), face(FaceName::Annotation))]);
                    }
                    for (staged, group) in [(true, "Staged"), (false, "Changes")] {
                        let files: Vec<_> = (status.files.iter())
                            .filter_map(|f| Some((if staged { f.staged } else { f.unstaged }?, f)))
                            .collect();
                        if files.is_empty() {
                            continue;
                        }
                        lines.push(vec![
                            ("    ".into(), None),
                            (group.into(), face(FaceName::DiffListGroup)),
                            (
                                format!("  {}", files.len()),
                                face(FaceName::SessionListCount),
                            ),
                        ]);
                        for (change, file) in files {
                            let full = status.repository.join(&file.path);
                            let path = (full.strip_prefix(root))
                                .map_or_else(|_| file.path.clone(), Path::to_path_buf);
                            let (letter, letter_face) = letter(change);
                            lines.push(vec![
                                ("      ".into(), None),
                                (letter.into(), face(letter_face)),
                                (format!("  {}", path.display()), None),
                            ]);
                            let root = root.clone();
                            rows.push((lines.len() - 1, Row::File { root, staged, path }));
                        }
                    }
                }
            }
        }
        let target = (at.and_then(|at| rows.iter().find(|(_, r)| *r == at)))
            .or_else(|| rows.iter().find(|(_, r)| matches!(r, Row::File { .. })))
            .map(|(l, _)| *l);
        let buffer = self.buffers.get_mut(id).expect("checked above");
        buffer.diff_list_mut().expect("checked above").rows = rows;
        let text: Vec<String> = (lines.iter())
            .map(|line| line.iter().map(|(text, _)| text.as_str()).collect())
            .collect();
        buffer.remove_properties(0..buffer.len_chars());
        buffer.set_text(&text.join("\n"));
        let mut at = 0;
        for line in lines {
            for (text, prop) in line {
                let end = at + text.chars().count();
                if let Some(prop) = prop.filter(|_| end > at) {
                    (buffer.add_property(at..end, prop)).expect("in the text");
                }
                at = end;
            }
            at += 1;
        }
        let point = target.map_or(0, |line| buffer.char_idx(line, 0).expect("a row's line"));
        buffer.set_marker(buffer.point, point).expect("in the text");
        let windows = (self.windows.windows_mut()).chain(
            (self.workspaces.iter_mut())
                .flat_map(|w| w.windows.iter_mut())
                .flat_map(|w| w.windows_mut()),
        );
        for window in windows.filter(|w| w.buffer() == id) {
            window
                .set_point(&mut self.buffers, point)
                .expect("in the text");
        }
    }
}

impl Ctx<'_> {
    /// `diff-status`: saves the workspace's layout and shows its changed-file list beside the
    /// current buffer; while the view is open, selects the list.
    pub(crate) fn diff_status(&mut self) -> Result<(), String> {
        self.outside_minibuffer("Can't show a diff in the minibuffer")?;
        let core = &mut *self.core;
        let name = format!("*Diff*<{}>", core.current_workspace()?.name);
        let found = core.diff_list_here();
        let list = found.unwrap_or_else(|| {
            let id = core.buffers.create(&name);
            let buffer = core.buffers.get_mut(id).expect("just created");
            (buffer.major_mode, buffer.read_only) = (MODE, true);
            buffer.kind = Kind::DiffList(DiffList {
                roots: Vec::new(),
                answer: None,
                pending: 0,
                rows: Vec::new(),
            });
            id
        });
        if core.windows.saved.is_some() {
            let shown = (core.windows.others.iter()).find(|(_, w)| w.buffer() == list);
            match shown.map(|(id, _)| *id) {
                Some(id) => core.select(id),
                None => core.show_buffer(list),
            }
        } else {
            let (left, right) = (WindowId(core.next_window), WindowId(core.next_window + 1));
            core.next_window += 2;
            let diff = core.windows.window.split(&mut core.buffers);
            let mut view = Windows::new(Window::new(&mut core.buffers, list), left);
            view.others.insert(right, diff);
            view.tree = Tree::Split {
                side_by_side: true,
                ratio: RATIO,
                first: Box::new(Tree::Leaf(left)),
                second: Box::new(Tree::Leaf(right)),
            };
            core.pending_visit = None;
            let saved = std::mem::replace(&mut core.windows, view);
            core.windows.saved = Some(Box::new(saved));
            core.buffers.raise(list);
        }
        match found {
            Some(_) => Ok(()),
            None => {
                core.render_diff_list(list);
                self.diff_refresh()
            }
        }
    }

    /// `diff-refresh`: asks for the status of the list's roots again.
    pub(crate) fn diff_refresh(&mut self) -> Result<(), String> {
        let roots = self.core.roots();
        let id = self.core.diff_list_here().ok_or("No diff view")?;
        let buffer = self.core.buffers.get_mut(id).expect("found live");
        let list = buffer.diff_list_mut().expect("found by kind");
        list.pending = if list.roots == roots {
            list.pending + 1
        } else {
            1
        };
        list.roots = roots.clone();
        self.effects.push(Effect::File(FileOp::GitStatus(roots)));
        Ok(())
    }

    /// `diff-quit`: restores the layout the view replaced and kills the list.
    pub(crate) fn diff_quit(&mut self) -> Result<(), String> {
        self.outside_minibuffer("Can't quit the diff view from the minibuffer")?;
        let core = &mut *self.core;
        let saved = core.windows.saved.take().ok_or("No diff view")?;
        let mut view = std::mem::replace(&mut core.windows, *saved);
        view.windows_mut()
            .for_each(|w| w.release(&mut core.buffers));
        core.pending_visit = None;
        if let Some(list) = core.diff_list_here() {
            core.kill_buffer(list);
        }
        core.buffers.raise(core.windows.window.buffer());
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::Args;
    use crate::harness::Harness;
    use crate::project::Root;
    use crate::{GitFile, Msg};

    /// A harness whose project has roots `a`, `b`, `c` and `d`, in the view.
    fn view() -> Harness {
        let mut h = Harness::new(&[]);
        let project = &mut h.core.projects[0].project;
        project.primary.path = h.dir.join("a");
        project.roots = ["b", "c", "d", "e"]
            .map(|r| Root {
                path: h.dir.join(r),
            })
            .into();
        h.call("diff-status", Args::default());
        h
    }

    fn file(path: &str, staged: Option<GitChange>, unstaged: Option<GitChange>) -> GitFile {
        GitFile {
            path: path.into(),
            staged,
            unstaged,
            old_path: None,
        }
    }

    /// `a` on `main` with `src/x.rs` staged and changed again and `notes` untracked (its
    /// repository one above it), `b` on `topic` with `gone` deleted, `c` clean, `d` outside git, `e` failing.
    fn answer(h: &Harness, extra: &[GitFile]) -> Msg {
        use GitChange::*;
        let status = |repository: &Path, branch: &str, files: Vec<GitFile>| {
            let (repository, branch) = (repository.to_path_buf(), branch.into());
            Ok(Some(GitStatus {
                repository,
                branch,
                files,
            }))
        };
        let mut a = vec![
            file("a/src/x.rs", Some(Modified), Some(Modified)),
            file("a/notes", None, Some(Untracked)),
        ];
        a.extend(extra.iter().cloned());
        Msg::GitStatus(vec![
            (h.dir.join("a"), status(&h.dir, "main", a)),
            (
                h.dir.join("b"),
                status(
                    &h.dir.join("b"),
                    "topic",
                    vec![file("gone", None, Some(Deleted))],
                ),
            ),
            (
                h.dir.join("c"),
                status(&h.dir.join("c"), "main", Vec::new()),
            ),
            (h.dir.join("d"), Ok(None)),
            (h.dir.join("e"), Err("fatal: x\n  y".into())),
        ])
    }

    fn shown(h: &Harness) -> Vec<(WindowId, String)> {
        let window = |id| (h.core.windows.others.get(&id)).unwrap_or(&h.core.windows.window);
        (h.core.windows.tree.leaves().into_iter())
            .map(|id| {
                (
                    id,
                    h.core
                        .buffers
                        .get(window(id).buffer())
                        .unwrap()
                        .name
                        .clone(),
                )
            })
            .collect()
    }

    #[test]
    fn the_list_groups_each_roots_files_under_its_branch() {
        let mut h = view();
        assert_eq!(h.text(), "Reading status...");
        let answer = answer(&h, &[]);
        h.send(answer);
        assert_eq!(
            h.text(),
            [
                "  a  ⎇ main",
                "    Staged  1",
                "      M  src/x.rs",
                "    Changes  2",
                "      M  src/x.rs",
                "      ?  notes",
                "",
                "  b  ⎇ topic",
                "    Changes  1",
                "      D  gone",
                "",
                "  c  ⎇ main",
                "    No changes",
                "",
                "  d  not a git repository",
                "",
                "  e  fatal: x y",
            ]
            .join("\n")
        );
        let first = h.text().find("      M").unwrap();
        assert_eq!(
            h.point(),
            h.text()[..first].chars().count(),
            "on the first file"
        );
    }

    #[test]
    fn the_view_replaces_the_layout_and_quitting_restores_it() {
        let mut h = Harness::new(&[]);
        h.keys("C-x 2 C-x 3 C-x o");
        let before = shown(&h);
        let selected = h.core.windows.selected;
        h.call("diff-status", Args::default());
        let view = shown(&h);
        assert_eq!(view.len(), 2);
        assert_eq!(view[0].1, "*Diff*<main>");
        assert_eq!(view[1].1, before[1].1, "the current buffer on the right");
        assert_eq!(h.core.windows.selected, view[0].0, "the list is selected");
        h.keys("C-x o");
        h.call("diff-status", Args::default());
        assert_eq!(
            h.core.windows.selected, view[0].0,
            "running it again selects the list"
        );
        assert_eq!(shown(&h), view);
        h.keys("C-x o C-c q");
        assert_eq!(shown(&h), before);
        assert_eq!(h.core.windows.selected, selected);
        assert!(
            h.core.buffers.find_here("*Diff*<main>").is_none(),
            "the list is killed"
        );
        h.call("diff-status", Args::default());
        h.keys("q");
        assert_eq!(shown(&h), before, "q in the list quits");
    }

    #[test]
    fn the_view_needs_a_workspace_and_travels_with_it() {
        let mut h = Harness::lobby(&[]);
        h.call("diff-status", Args::default());
        assert_eq!(h.core.message(), Some("No workspace"));
        let mut h = Harness::new(&[]);
        let main = h.core.workspace().unwrap();
        h.call("diff-status", Args::default());
        let view = shown(&h);
        h.new_workspace("other", "other");
        assert_eq!(shown(&h).len(), 1);
        h.core.switch_workspace(main).unwrap();
        assert_eq!(shown(&h), view);
        h.keys("q");
        assert_eq!(shown(&h).len(), 1, "the saved layout came back with it");
    }

    #[test]
    fn a_buffer_killed_in_the_view_leaves_the_saved_layout() {
        let mut h = Harness::new(&[("f", "x")]);
        h.open("f");
        h.keys("C-x 2");
        h.call("diff-status", Args::default());
        let f = h.core.buffers.find_here("f").unwrap();
        h.core.kill_buffer(f);
        h.keys("q");
        let names: Vec<_> = shown(&h).into_iter().map(|(_, name)| name).collect();
        assert_eq!(names, ["*scratch*<main>", "*scratch*<main>"]);
        h.frame(800., 600.);
    }

    #[test]
    fn the_list_changes_only_on_refresh_keeping_point_on_its_file() {
        let mut h = view();
        let first = answer(&h, &[]);
        h.send(first);
        h.keys("C-n C-n C-n");
        let on = |h: &Harness| {
            let line = h.core.current().line_col(h.point()).unwrap().0;
            h.text().lines().nth(line).unwrap().to_string()
        };
        assert_eq!(on(&h), "      ?  notes");
        let earlier = [file("a/b.rs", None, Some(GitChange::Added))];
        let text = h.text();
        let unasked = answer(&h, &earlier);
        h.send(unasked);
        assert_eq!(h.text(), text, "no refresh, no change");
        h.keys("g");
        let refreshed = answer(&h, &earlier);
        h.send(refreshed);
        assert!(h.text().contains("A  b.rs"));
        assert_eq!(on(&h), "      ?  notes");
    }

    #[test]
    fn one_mode_line_runs_under_both_windows() {
        let mut h = view();
        let frame = h.frame(1440., 900.);
        let [list, right] = &frame.windows[..] else {
            panic!("two windows");
        };
        assert_eq!(list.bar, None);
        let (bar, active) = right.bar.unwrap();
        assert_eq!((bar.x, bar.x + bar.width), (0., 1440.));
        assert!(active, "active while the list is selected");
        assert!(
            list.rect.width < right.rect.width / 2.,
            "the list is narrow"
        );
    }
}
