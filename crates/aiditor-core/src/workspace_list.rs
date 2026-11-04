//! A temporary tree of projects, workspaces and their buffers.

use crate::{Ctx, project::ProjectId, workspace::WorkspaceId};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Row {
    Project(ProjectId),
    Workspace(WorkspaceId),
}

const MODE: &str = "workspace-list-mode";

impl Ctx<'_> {
    pub(crate) fn list_workspaces(&mut self) -> Result<(), String> {
        self.outside_minibuffer("Can't list workspaces in the minibuffer")?;
        let core = &mut *self.core;
        let current = core.workspace();
        let mut projects = Vec::new();
        for workspace in &core.workspaces {
            if !projects.contains(&workspace.project) {
                projects.push(workspace.project);
            }
        }
        let mut empty: Vec<_> = core
            .projects
            .iter()
            .filter(|p| !projects.contains(&p.id))
            .collect();
        empty.sort_by(|a, b| a.project.name.cmp(&b.project.name));
        projects.extend(empty.into_iter().map(|p| p.id));
        let mut lines = vec![
            "Workspaces — RET switch, n/p move, q quit".into(),
            String::new(),
        ];
        let mut rows = Vec::new();
        for project_id in &projects {
            let project = &core.projects[core.project_at(*project_id)?].project;
            let workspaces: Vec<_> = core
                .workspaces
                .iter()
                .filter(|w| w.project == *project_id)
                .collect();
            rows.push((lines.len(), Row::Project(*project_id)));
            let empty = if workspaces.is_empty() {
                " · no workspaces"
            } else {
                ""
            };
            lines.push(format!(
                "{} ({}){empty}",
                project.name.escape_debug(),
                project.primary.path.to_string_lossy().escape_debug()
            ));
            for (index, workspace) in workspaces.iter().enumerate() {
                let last = index + 1 == workspaces.len();
                let branch = if last { "└──" } else { "├──" };
                let mark = if workspace.id == current { "* " } else { "" };
                rows.push((lines.len(), Row::Workspace(workspace.id)));
                lines.push(format!("{branch} {mark}{}", workspace.name.escape_debug()));
                let mut buffers: Vec<_> = core
                    .buffers
                    .recent()
                    .filter(|(_, b)| {
                        b.workspace == Some(workspace.id)
                            && !b.name.starts_with(' ')
                            && b.major_mode != MODE
                    })
                    .map(|(_, b)| b.name.as_str())
                    .collect();
                buffers.sort_unstable();
                for (index, name) in buffers.iter().enumerate() {
                    let prefix = if last { "    " } else { "│   " };
                    let branch = if index + 1 == buffers.len() {
                        "└──"
                    } else {
                        "├──"
                    };
                    lines.push(format!("{prefix}{branch} {}", name.escape_debug()));
                }
            }
        }
        let found = core
            .buffers
            .recent()
            .find(|(_, b)| b.major_mode == MODE && b.workspace == Some(current))
            .map(|(id, _)| id);
        let id = found.unwrap_or_else(|| core.buffers.create("*Workspaces*"));
        let buffer = core.buffers.get_mut(id).expect("list buffer is live");
        buffer.major_mode = MODE;
        buffer.read_only = true;
        buffer.set_text(&lines.join("\n"));
        let line = rows
            .iter()
            .find(|(_, row)| *row == Row::Workspace(current))
            .expect("current workspace is listed")
            .0;
        buffer.workspace_rows = rows;
        let point = buffer.char_idx(line, 0)?;
        core.switch_to(id);
        self.goto(point)
    }

    fn workspace_list_rows(&self) -> Result<&[(usize, Row)], String> {
        let buffer = self.core.current();
        if buffer.major_mode != MODE || buffer.workspace_rows.is_empty() {
            return Err("Not in a workspace list".into());
        }
        Ok(&buffer.workspace_rows)
    }

    pub(crate) fn workspace_list_move(&mut self, forward: bool) -> Result<(), String> {
        let rows = self.workspace_list_rows()?;
        let line = self.core.current().line_col(self.core.point())?.0;
        let target = if forward {
            rows.iter()
                .find(|(l, _)| *l > line)
                .unwrap_or(rows.last().expect("nonempty"))
        } else {
            rows.iter()
                .rev()
                .find(|(l, _)| *l < line)
                .unwrap_or(&rows[0])
        };
        self.goto(self.core.current().char_idx(target.0, 0)?)
    }

    pub(crate) fn workspace_list_visit(&mut self) -> Result<(), String> {
        let rows = self.workspace_list_rows()?;
        let line = self.core.current().line_col(self.core.point())?.0;
        let id = rows
            .iter()
            .find(|(l, _)| *l == line)
            .ok_or("Select a project or workspace row")?
            .1;
        match id {
            Row::Workspace(id) => {
                if !self.core.workspaces.iter().any(|w| w.id == id) {
                    return Err("No such workspace".into());
                }
                self.workspace_list_quit()?;
                self.core.switch_workspace(id)
            }
            Row::Project(id) => {
                self.core.project_at(id)?;
                self.workspace_list_quit()?;
                self.core.switch_project(id)
            }
        }
    }

    pub(crate) fn workspace_list_quit(&mut self) -> Result<(), String> {
        self.workspace_list_rows()?;
        let id = self.core.windows.window.buffer();
        self.core.kill_buffer(id);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{harness::Harness, project::Loaded};

    fn workspaces() -> Harness {
        let mut h = Harness::new(&[]);
        let launch = h.core.workspace();
        h.new_workspace("a", "p");
        h.new_workspace("b", "p");
        h.open("notes.txt");
        h.new_workspace("c", "q");
        h.core.kill_workspace(launch).unwrap();
        h.core.projects.retain(|p| p.project.name != "home");
        h.core.switch_to_workspace("b").unwrap();
        h
    }

    fn row(h: &Harness) -> Row {
        let line = h.core.current().line_col(h.point()).unwrap().0;
        h.core
            .current()
            .workspace_rows
            .iter()
            .find(|(l, _)| *l == line)
            .unwrap()
            .1
    }

    #[test]
    fn tree_groups_workspaces_and_only_their_listed_buffers() {
        let mut h = workspaces();
        h.core.buffers.create(" hidden");
        h.keys("C-g C-x t l");
        assert_eq!(
            h.text(),
            "Workspaces — RET switch, n/p move, q quit\n\np (/aiditor-test)\n├── a\n│   └── *scratch*<a>\n└── * b\n    ├── *scratch*<b>\n    └── notes.txt\nq (/aiditor-test)\n└── c\n    └── *scratch*<c>"
        );
        assert_eq!(row(&h), Row::Workspace(h.core.workspace()));
        assert!(h.core.current().read_only);
        assert_eq!(
            (h.core.current().file.as_ref(), h.core.current().modified()),
            (None, false)
        );
        let text = h.text();
        h.keys("x");
        assert_eq!(h.core.message(), Some("Buffer is read-only"));
        assert_eq!(h.text(), text);
    }

    #[test]
    fn equal_project_names_and_multiline_labels_keep_their_identity() {
        let mut h = Harness::new(&[]);
        let launch = h.core.workspace();
        let first = h.core.add_project(Loaded::ad_hoc("app", &h.path("first")));
        let second = h.core.add_project(Loaded::ad_hoc("app", &h.path("second")));
        let a = h.core.new_workspace("a\nnext", first);
        let b = h.core.new_workspace("b", second);
        h.core.buffers.create("tab\tname");
        h.core.kill_workspace(launch).unwrap();
        h.core.projects.retain(|p| p.project.name != "home");
        h.keys("C-x t l");
        assert_eq!(
            h.text(),
            format!(
                "Workspaces — RET switch, n/p move, q quit\n\napp ({})\n└── a\\nnext\n    └── *scratch*<a\\nnext>\napp ({})\n└── * b\n    ├── *scratch*<b>\n    └── tab\\tname",
                h.path("first").display(),
                h.path("second").display()
            )
        );
        assert_eq!(row(&h), Row::Workspace(b));
        h.keys("p p RET");
        assert_eq!(h.core.workspace(), a);
    }

    #[test]
    fn empty_projects_are_sorted_last_and_activation_creates_the_first_workspace() {
        let mut h = workspaces();
        h.core.add_project(Loaded::ad_hoc("zeta", &h.path("zeta")));
        let alpha = h
            .core
            .add_project(Loaded::ad_hoc("alpha", &h.path("alpha")));
        h.keys("C-x t l");
        assert!(h.text().ends_with(
            "alpha (/aiditor-test/alpha) · no workspaces\nzeta (/aiditor-test/zeta) · no workspaces"
        ));
        h.keys("n n n RET");
        assert_eq!(h.core.current_workspace().project, alpha);
        assert_eq!(h.core.current_workspace().name, "alpha");
        assert_eq!(h.core.current().name, "*scratch*<alpha>");
        h.core.switch_to_workspace("b").unwrap();
        assert_eq!(h.core.current().name, "notes.txt");
    }

    #[test]
    fn project_activation_uses_identity_and_the_most_recent_workspace() {
        let mut h = workspaces();
        let project = h.core.current_workspace().project;
        let other = h.core.add_project(Loaded::ad_hoc("p", &h.path("other")));
        h.core.new_workspace("other", other);
        h.core.switch_to_workspace("a").unwrap();
        h.keys("body C-x 2 C-b");
        let point = h.point();
        let selected = h.core.windows.selected;
        h.core.switch_to_workspace("other").unwrap();
        h.keys("C-x t l M-< n RET");
        assert_eq!(h.core.current_workspace().project, project);
        assert_eq!(h.core.current_workspace().name, "a");
        assert_eq!(h.core.windows.tree.leaves().len(), 2);
        assert_eq!(h.core.windows.selected, selected);
        assert_eq!((h.text(), h.point()), ("body".into(), point));
        h.keys("C-x t l M-< n RET");
        assert_eq!(h.text(), "body");
    }

    #[test]
    fn stale_project_activation_keeps_the_list_available() {
        let mut h = workspaces();
        let empty = h
            .core
            .add_project(Loaded::ad_hoc("empty", &h.path("empty")));
        h.keys("C-x t l M-> n");
        let list = h.core.windows.window.buffer();
        h.core.projects.retain(|p| p.id != empty);
        h.keys("RET");
        assert_eq!(h.core.message(), Some("Project is no longer loaded"));
        assert_eq!(h.core.windows.window.buffer(), list);
    }

    #[test]
    fn mode_line_counts_snapshot_rows_and_refreshes_in_every_window() {
        let mut h = workspaces();
        for name in ["alpha", "beta", "gamma"] {
            h.core.add_project(Loaded::ad_hoc(name, &h.path(name)));
        }
        h.new_workspace("d", "q");
        h.keys("C-x t l M-< n n");
        let line = h.frame(1400., 600.).windows[0].display.mode_line.text();
        assert_eq!(line, "*Workspaces* │ 5 projects, 4 workspaces  2/9");
        h.keys("C-f C-n");
        h.keys("C-p C-e C-f");
        assert_eq!(
            h.frame(1400., 600.).windows[0].display.mode_line.text(),
            line
        );
        h.core
            .add_project(Loaded::ad_hoc("delta", &h.path("delta")));
        assert_eq!(
            h.frame(1400., 600.).windows[0].display.mode_line.text(),
            line
        );
        h.keys("C-x t l C-x 2");
        for window in h.frame(1400., 600.).windows {
            assert_eq!(
                window.display.mode_line.text(),
                "*Workspaces* │ 6 projects, 4 workspaces  6/10"
            );
        }
    }

    #[test]
    fn navigation_skips_context_rows_and_stops_at_the_ends() {
        for (next, previous) in [("n", "p"), ("C-n", "C-p"), ("<down>", "<up>")] {
            let mut h = workspaces();
            h.keys("C-x t l M-<").keys(previous);
            let expected = vec![
                Row::Project(h.core.workspaces[0].project),
                Row::Workspace(h.core.workspaces[0].id),
                Row::Workspace(h.core.workspaces[1].id),
                Row::Project(h.core.workspaces[2].project),
                Row::Workspace(h.core.workspaces[2].id),
            ];
            for target in &expected {
                assert_eq!(row(&h), *target);
                h.keys(next);
            }
            assert_eq!(row(&h), *expected.last().unwrap());
            for target in expected.iter().rev() {
                assert_eq!(row(&h), *target);
                h.keys(previous);
            }
            assert_eq!(row(&h), expected[0]);
            h.keys("M->").keys(next);
            assert_eq!(row(&h), *expected.last().unwrap());
        }
    }

    #[test]
    fn moving_in_a_long_tree_keeps_the_workspace_row_visible() {
        let mut h = Harness::new(&[]);
        for n in 0..20 {
            h.new_workspace(&format!("w{n}"), "p");
        }
        h.keys("C-x t l");
        for _ in 0..22 {
            let display = h.display(5);
            let (line, _) = display.cursor.expect("selected row visible");
            assert_eq!(
                display.lines[line].number,
                h.core.current().line_col(h.point()).unwrap().0
            );
            h.keys("p");
        }
        assert_eq!(row(&h), Row::Project(h.core.workspaces[0].project));
        let display = h.display(5);
        assert!(display.cursor.is_some());
    }

    #[test]
    fn activation_restores_the_destination_layout_and_dismisses_the_list() {
        let mut h = workspaces();
        h.core.switch_to_workspace("a").unwrap();
        h.keys("target C-x 2 C-b");
        let a = h.core.workspace();
        let selected = h.core.windows.selected;
        let point = h.point();
        h.core.switch_to_workspace("b").unwrap();
        let b = h.core.workspace();
        h.keys("C-x t l");
        h.keys("p RET");
        assert_eq!(h.core.workspace(), a);
        assert_eq!(h.core.windows.selected, selected);
        assert_eq!(h.core.windows.tree.leaves().len(), 2);
        assert_eq!((h.text(), h.point()), ("target".into(), point));
        h.core.switch_workspace(b).unwrap();
        assert_eq!(h.core.current().name, "notes.txt");
    }

    #[test]
    fn informational_and_stale_rows_leave_the_list_available() {
        let mut h = workspaces();
        h.keys("C-x t l");
        let workspace = h.core.workspace();
        let list = h.core.windows.window.buffer();
        for line in [0, 4] {
            let at = h.core.current().char_idx(line, 0).unwrap();
            let window = &h.core.windows.window;
            window.set_point(&mut h.core.buffers, at).unwrap();
            h.keys("RET");
            assert_eq!(h.core.message(), Some("Select a project or workspace row"));
            assert_eq!(
                (h.core.workspace(), h.core.windows.window.buffer()),
                (workspace, list)
            );
        }
        h.keys("n");
        assert_eq!(row(&h), Row::Workspace(workspace));
        h.keys("p");
        let Row::Workspace(target) = row(&h) else {
            panic!("workspace selected")
        };
        h.core.kill_workspace(target).unwrap();
        h.keys("RET");
        assert_eq!(h.core.message(), Some("No such workspace"));
        assert_eq!(
            (h.core.workspace(), h.core.windows.window.buffer()),
            (workspace, list)
        );
    }

    #[test]
    fn dismissing_returns_to_ordinary_editing_with_text_and_splits_intact() {
        for quit in ["q", "C-g", "RET", "C-x k RET"] {
            let mut h = Harness::new(&[]);
            h.keys("body C-x 2 M-x workspace-list RET");
            h.keys(quit);
            assert_eq!(h.text(), "body");
            assert_eq!(h.core.windows.tree.leaves().len(), 2);
            h.keys("npq");
            assert_eq!(h.text(), "bodynpq");
            h.keys("C-x t l");
            assert_eq!(h.core.current().name, "*Workspaces*");
            assert_eq!(row(&h), Row::Workspace(h.core.workspace()));
        }
    }

    #[test]
    fn refresh_reuses_the_list_and_workspace_teardown_releases_it() {
        let mut h = workspaces();
        let b = h.core.workspace();
        h.keys("C-x t l");
        let list = h.core.windows.window.buffer();
        h.keys("C-x t o");
        h.keys("C-x t l");
        let other_list = h.core.windows.window.buffer();
        assert_eq!(h.core.current().name, "*Workspaces*<2>");
        h.core.switch_workspace(b).unwrap();
        assert_eq!(h.core.windows.window.buffer(), list);
        h.open("added.txt");
        h.keys("C-x t l");
        assert_eq!(h.core.windows.window.buffer(), list);
        assert!(h.text().contains("    ├── added.txt\n    └── notes.txt"));
        let notes = h.core.buffers.find_here("notes.txt").unwrap();
        h.core.kill_buffer(notes);
        h.core.rename_workspace("renamed").unwrap();
        h.keys("C-x t l");
        assert!(
            h.text()
                .contains("└── * renamed\n    ├── *scratch*<renamed>\n    └── added.txt\nq (")
        );
        h.core.kill_workspace(b).unwrap();
        assert_eq!(h.core.windows.window.buffer(), other_list);
        h.keys("C-x t l q C-x t l");
        assert_eq!(h.core.current().name, "*Workspaces*");
        assert_eq!(h.core.current().workspace_rows.len(), 4);
    }
}
