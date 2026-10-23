//! A temporary tree of projects, workspaces and their buffers.

use crate::{Ctx, workspace::WorkspaceId};

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
        let mut lines = vec![
            "Workspaces — RET switch, n/p move, q quit".into(),
            String::new(),
        ];
        let mut rows = Vec::new();
        for project_id in &projects {
            let project = &core.projects[core.project_at(*project_id)?].project;
            let duplicate = projects
                .iter()
                .any(|id| id != project_id && core.project_name(*id) == project.name);
            lines.push(if duplicate {
                format!(
                    "{} ({})",
                    project.name.escape_debug(),
                    project.primary.path.to_string_lossy().escape_debug()
                )
            } else {
                project.name.escape_debug().to_string()
            });
            let workspaces: Vec<_> = core
                .workspaces
                .iter()
                .filter(|w| w.project == *project_id)
                .collect();
            for (index, workspace) in workspaces.iter().enumerate() {
                let last = index + 1 == workspaces.len();
                let branch = if last { "└──" } else { "├──" };
                let mark = if workspace.id == current { "* " } else { "" };
                rows.push((lines.len(), workspace.id));
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
            .find(|(_, id)| *id == current)
            .expect("current workspace is listed")
            .0;
        buffer.workspace_rows = rows;
        let point = buffer.char_idx(line, 0)?;
        core.switch_to(id);
        self.goto(point)
    }

    fn workspace_list_rows(&self) -> Result<&[(usize, WorkspaceId)], String> {
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
            .ok_or("Select a workspace row")?
            .1;
        if !self.core.workspaces.iter().any(|w| w.id == id) {
            return Err("No such workspace".into());
        }
        self.workspace_list_quit()?;
        self.core.switch_workspace(id)
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
    use crate::{Msg, harness::Harness, project::Loaded};

    fn workspaces() -> Harness {
        let mut h = Harness::new(&[]);
        let launch = h.core.workspace();
        h.new_workspace("a", "p");
        h.new_workspace("b", "p");
        h.open("notes.txt");
        h.new_workspace("c", "q");
        h.core.kill_workspace(launch).unwrap();
        h.core.switch_to_workspace("b").unwrap();
        h
    }

    fn row(h: &Harness) -> WorkspaceId {
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
            "Workspaces — RET switch, n/p move, q quit\n\np\n├── a\n│   └── *scratch*<a>\n└── * b\n    ├── *scratch*<b>\n    └── notes.txt\nq\n└── c\n    └── *scratch*<c>"
        );
        assert_eq!(row(&h), h.core.workspace());
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
        h.keys("C-x t l");
        assert_eq!(
            h.text(),
            format!(
                "Workspaces — RET switch, n/p move, q quit\n\napp ({})\n└── a\\nnext\n    └── *scratch*<a\\nnext>\napp ({})\n└── * b\n    ├── *scratch*<b>\n    └── tab\\tname",
                h.path("first").display(),
                h.path("second").display()
            )
        );
        assert_eq!(row(&h), b);
        h.keys("p RET");
        assert_eq!(h.core.workspace(), a);
    }

    #[test]
    fn navigation_skips_context_rows_and_stops_at_the_ends() {
        for (next, previous) in [("n", "p"), ("C-n", "C-p"), ("<down>", "<up>")] {
            let mut h = workspaces();
            let ids: Vec<_> = h.core.workspaces.iter().map(|w| w.id).collect();
            h.keys("C-x t l");
            h.keys(next);
            assert_eq!(row(&h), ids[2]);
            h.keys(next);
            assert_eq!(row(&h), ids[2]);
            h.keys(previous).keys(previous).keys(previous);
            assert_eq!(row(&h), ids[0]);
            h.keys("M-<").keys(previous);
            assert_eq!(row(&h), ids[0]);
            h.keys("M->").keys(next);
            assert_eq!(row(&h), ids[2]);
        }
    }

    #[test]
    fn moving_in_a_long_tree_keeps_the_workspace_row_visible() {
        let mut h = Harness::new(&[]);
        for n in 0..20 {
            h.new_workspace(&format!("w{n}"), "p");
        }
        h.keys("C-x t l");
        for _ in 0..20 {
            let display = h.display(5);
            let (line, _) = display.cursor.expect("selected row visible");
            assert_eq!(
                display.lines[line].number,
                h.core.current().line_col(h.point()).unwrap().0
            );
            h.keys("p");
        }
        assert_eq!(row(&h), h.core.workspaces[0].id);
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
        for line in [2, 4] {
            h.send(Msg::Click {
                window: h.core.windows.selected,
                line,
                col: 0,
            });
            h.keys("RET");
            assert_eq!(h.core.message(), Some("Select a workspace row"));
            assert_eq!(
                (h.core.workspace(), h.core.windows.window.buffer()),
                (workspace, list)
            );
        }
        h.keys("n");
        assert_eq!(row(&h), workspace);
        h.keys("p");
        let target = row(&h);
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
            assert_eq!(row(&h), h.core.workspace());
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
                .contains("└── * renamed\n    ├── *scratch*<renamed>\n    └── added.txt\nq")
        );
        h.core.kill_workspace(b).unwrap();
        assert_eq!(h.core.windows.window.buffer(), other_list);
        h.keys("C-x t l q C-x t l");
        assert_eq!(h.core.current().name, "*Workspaces*");
        assert_eq!(h.core.current().workspace_rows.len(), 2);
    }
}
