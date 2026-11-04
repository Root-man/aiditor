//! A temporary tree of projects, workspaces and their buffers.

use crate::{
    Core, Ctx,
    buffer::{Buffer, FaceRef, Prop},
    project::ProjectId,
    settings::FaceName,
    workspace::WorkspaceId,
};
use std::path::PathBuf;

struct Entry {
    name: String,
    column: usize,
    face: Vec<FaceRef>,
    location: String,
    state: Vec<(String, FaceName)>,
}

impl Row {
    fn column(self) -> usize {
        match self {
            Self::Project(_) => 2,
            Self::Workspace(_) => 6,
        }
    }
}

impl Entry {
    fn render(self, buffer: &mut Buffer) {
        let start = buffer.len_chars() + 1;
        let name_len = self.name.chars().count();
        let location = self.location.escape_debug().to_string();
        let state_len: usize = self.state.iter().map(|(s, _)| s.chars().count()).sum();
        let mut line = format!(
            "{} {}{}   {}",
            self.name,
            location,
            " ".repeat(46usize.saturating_sub(location.chars().count())),
            " ".repeat(26usize.saturating_sub(state_len))
        );
        let state_at = start + line.chars().count();
        for (text, _) in &self.state {
            line.push_str(text);
        }
        buffer.insert_output(start - 1, &format!("\n{line}"));
        let mut named = |range, refs| {
            buffer
                .add_property(range, Prop::Named(refs))
                .expect("just inserted")
        };
        named(
            start..start + self.column,
            vec![FaceRef::Face(FaceName::Annotation)],
        );
        let face_start = if self.name.starts_with("    * ") {
            4
        } else {
            self.column
        };
        named(start + face_start..start + name_len, self.face);
        named(
            start + name_len + 1..state_at,
            vec![FaceRef::Face(FaceName::Annotation)],
        );
        let mut at = state_at;
        for (text, face) in self.state {
            let end = at + text.chars().count();
            buffer
                .add_property(at..end, Prop::Named(vec![FaceRef::Face(face)]))
                .expect("just inserted");
            at = end;
        }
        buffer
            .add_property(start + name_len..start + name_len + 1, Prop::AlignRight)
            .expect("just inserted");
    }
}

fn details(
    core: &Core,
    id: crate::buffer::BufferId,
    buffer: &Buffer,
    roots: &[PathBuf],
) -> (String, Vec<(String, FaceName)>) {
    if let Some(session) = core.agents.values().find(|s| s.buffer == id) {
        let location = session.model_effort().into_iter().map(|(s, _)| s).collect();
        let state = session
            .activity()
            .map(|a| (a.text().to_string(), a.face()))
            .into_iter()
            .collect();
        return (location, state);
    }
    if let Some(terminal) = buffer.terminal() {
        let line = crate::display::terminal_mode_line(buffer, terminal, 1, 0);
        let location = line
            .left
            .get(1)
            .map(|s| s.iter().map(|(t, _)| t.as_str()).collect())
            .unwrap_or_default();
        let state = line
            .right
            .into_iter()
            .flatten()
            .filter(|(_, f)| matches!(f, FaceName::StatusRunning | FaceName::StatusFailed))
            .collect();
        return (location, state);
    }
    let location = buffer
        .file
        .as_ref()
        .map(|file| {
            crate::display::file_mode_line(buffer, file, roots, None, (1, 0), "All").left[0]
                .iter()
                .filter(|(_, f)| *f == FaceName::ModeLineDim)
                .map(|(s, _)| s.as_str())
                .collect()
        })
        .unwrap_or_default();
    let mut state = Vec::new();
    for (shown, text, face) in [
        (buffer.modified(), "● unsaved", FaceName::StatusModified),
        (buffer.read_only, "⊘ read-only", FaceName::StatusFailed),
        (buffer.deleted, "[deleted]", FaceName::StatusFailed),
        (
            !buffer.deleted && buffer.conflict.is_some(),
            "[conflict]",
            FaceName::StatusFailed,
        ),
    ] {
        if shown {
            if !state.is_empty() {
                state.push((" ".into(), FaceName::Annotation));
            }
            state.push((text.into(), face));
        }
    }
    (location, state)
}

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
        let mut entries = Vec::new();
        let mut rows = Vec::new();
        for project_id in &projects {
            let project = &core.projects[core.project_at(*project_id)?].project;
            let roots: Vec<_> = project.all_roots().map(|r| r.path.clone()).collect();
            let workspaces: Vec<_> = core
                .workspaces
                .iter()
                .filter(|w| w.project == *project_id)
                .collect();
            rows.push((entries.len() + 2, Row::Project(*project_id)));
            let empty = if workspaces.is_empty() {
                " · no workspaces"
            } else {
                ""
            };
            entries.push(Entry {
                name: format!("  {}", project.name.escape_debug()),
                column: 2,
                face: vec![FaceRef::Bold],
                location: format!("{}{empty}", project.primary.path.to_string_lossy()),
                state: Vec::new(),
            });
            for workspace in workspaces {
                let mut buffers: Vec<_> = core
                    .buffers
                    .recent()
                    .filter(|(_, b)| {
                        b.workspace == Some(workspace.id)
                            && !b.name.starts_with(' ')
                            && b.major_mode != MODE
                    })
                    .collect();
                buffers.sort_unstable_by(|(_, a), (_, b)| a.name.cmp(&b.name));
                let unsaved = buffers
                    .iter()
                    .filter(|(_, b)| b.file.is_some() && b.modified())
                    .count();
                let mut location = format!(
                    "{} buffer{}",
                    buffers.len(),
                    if buffers.len() == 1 { "" } else { "s" }
                );
                if unsaved > 0 {
                    location.push_str(&format!(" · {unsaved} unsaved"));
                }
                let activity = core.workspace_activity(workspace.id);
                let current = workspace.id == current;
                rows.push((entries.len() + 2, Row::Workspace(workspace.id)));
                entries.push(Entry {
                    name: format!(
                        "    {}{}",
                        if current { "* " } else { "  " },
                        workspace.name.escape_debug()
                    ),
                    column: 6,
                    face: if current {
                        vec![FaceRef::Face(FaceName::WorkspaceCurrent), FaceRef::Bold]
                    } else {
                        Vec::new()
                    },
                    location,
                    state: activity
                        .map(|a| (a.text().to_string(), a.face()))
                        .into_iter()
                        .collect(),
                });
                let suffix = format!("<{}>", workspace.name);
                for (id, buffer) in buffers {
                    let name = buffer.name.strip_suffix(&suffix).unwrap_or(&buffer.name);
                    let (location, state) = details(core, id, buffer, &roots);
                    entries.push(Entry {
                        name: format!("      {}", name.escape_debug()),
                        column: 6,
                        face: vec![FaceRef::Face(FaceName::ModeLineDim)],
                        location,
                        state,
                    });
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
        buffer.remove_properties(0..buffer.len_chars());
        buffer.set_text("Workspaces — RET switch, n/p move, q quit\n");
        for entry in entries {
            entry.render(buffer);
        }
        buffer.set_saved();
        let line = rows
            .iter()
            .find(|(_, row)| *row == Row::Workspace(current))
            .expect("current workspace is listed")
            .0;
        buffer.workspace_rows = rows;
        let point = buffer.char_idx(line, Row::Workspace(current).column())?;
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
        self.goto(self.core.current().char_idx(target.0, target.1.column())?)
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

    fn contents(h: &Harness) -> Vec<String> {
        h.text()
            .lines()
            .map(|s| s.split_whitespace().collect::<Vec<_>>().join(" "))
            .collect()
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
    fn file_terminal_columns_and_faces_follow_the_snapshot_in_both_themes() {
        use crate::buffer::Decoration;
        for light in [false, true] {
            let mut h = Harness::new(&[
                ("src/clean.rs", "clean"),
                ("src/dirty.rs", "dirty"),
                ("vendor/locked.rs", "locked"),
                ("src/gone.rs", "gone"),
                ("src/conflict.rs", "conflict"),
            ]);
            h.new_workspace("task", "p");
            for name in [
                "src/clean.rs",
                "src/dirty.rs",
                "vendor/locked.rs",
                "src/gone.rs",
                "src/conflict.rs",
            ] {
                h.open(name);
            }
            let dirty = h.core.buffers.find_here("dirty.rs").unwrap();
            h.core
                .buffers
                .get_mut(dirty)
                .unwrap()
                .insert(0, "new ")
                .unwrap();
            let locked = h.core.buffers.find_here("locked.rs").unwrap();
            h.core.buffers.get_mut(locked).unwrap().read_only = true;
            let gone = h.core.buffers.find_here("gone.rs").unwrap();
            h.core.buffers.get_mut(gone).unwrap().deleted = true;
            let conflict = h.core.buffers.find_here("conflict.rs").unwrap();
            h.core.switch_to(dirty);
            h.core.buffers.get_mut(conflict).unwrap().conflict = Some(0);
            let (running, _) = h.core.create_terminal("vterm1<task>", 2, 10);
            let (failed, _) = h.core.create_terminal("vterm2<task>", 2, 10);
            for (id, bytes) in [
                (
                    running,
                    "\x1b]7;file://localhost/aiditor-test/app\x07\x1b]133;C;cmdline_url=cargo%20test\x07",
                ),
                (
                    failed,
                    "\x1b]7;file://localhost/aiditor-test/other\x07\x1b]133;D;101\x07",
                ),
            ] {
                h.send(crate::Msg::TerminalOutput {
                    id,
                    bytes: bytes.as_bytes().to_vec(),
                });
            }
            h.keys("C-x t l");
            let snapshot = h.text();
            assert_eq!(
                &contents(&h)[5..],
                [
                    "p /aiditor-test",
                    "* task 8 buffers · 1 unsaved",
                    "*scratch*",
                    "clean.rs src/",
                    "conflict.rs src/ [conflict]",
                    "dirty.rs src/ ● unsaved",
                    "gone.rs src/ [deleted]",
                    "locked.rs vendor/ ⊘ read-only",
                    "vterm1 /aiditor-test/app ◉ cargo test",
                    "vterm2 /aiditor-test/other exit 101"
                ]
            );
            if light {
                h.core.settings = crate::settings::Settings::parse(Some(
                    r#"
                    theme = "light"
                    [themes.light.faces]
                    default = { fg = 0x3c3836, bg = 0xfbf1c7 }
                    mode-line-dim = { fg = 0x7c6f64 }
                    annotation = { fg = 0x7c6f64 }
                    workspace-current = { fg = 0xb57614 }
                    completion-selected = { bg = 0xebdbb2 }
                    status-modified = { fg = 0xaf3a03 }
                    status-failed = { fg = 0x9d0006 }
                    status-running = { fg = 0x79740e }
                "#,
                ))
                .unwrap();
            }
            let theme = h.core.settings.theme.clone();
            let display = h.frame(1440., 900.).windows.remove(0).display;
            let spans: Vec<_> = display.lines.iter().flat_map(|l| &l.spans).collect();
            for (text, face, bold) in [
                ("p", FaceName::Default, true),
                ("* task", FaceName::WorkspaceCurrent, true),
                ("clean.rs", FaceName::ModeLineDim, false),
                ("src/", FaceName::Annotation, false),
                ("● unsaved", FaceName::StatusModified, false),
                ("⊘ read-only", FaceName::StatusFailed, false),
                ("[deleted]", FaceName::StatusFailed, false),
                ("[conflict]", FaceName::StatusFailed, false),
                ("◉ cargo test", FaceName::StatusRunning, false),
                ("exit 101", FaceName::StatusFailed, false),
            ] {
                assert!(
                    spans.iter().any(|s| s.text.trim() == text
                        && s.face.fg.or(theme.face(FaceName::Default).fg) == theme.face(face).fg
                        && s.face.bold == bold),
                    "{text}, light {light}"
                );
            }
            let (cursor_line, cursor_col) = display.cursor.unwrap();
            assert_eq!(cursor_col, 6);
            assert_eq!(display.lines[cursor_line].number, 6);
            assert!(
                display.lines[cursor_line]
                    .decorations
                    .contains(&Decoration::Background(FaceName::CompletionSelected))
            );
            let where_columns: Vec<_> = display
                .lines
                .iter()
                .filter(|l| [6, 8, 12, 13].contains(&l.number))
                .map(|l| {
                    let mut column = 0;
                    for span in &l.spans {
                        if [
                            "8 buffers · 1 unsaved",
                            "src/",
                            "vendor/",
                            "/aiditor-test/app",
                        ]
                        .iter()
                        .any(|t| span.text.starts_with(t))
                        {
                            return column;
                        }
                        column += span.text.chars().count();
                    }
                    panic!("location drawn");
                })
                .collect();
            assert_eq!(where_columns.len(), 4);
            assert!(where_columns.iter().all(|c| *c == where_columns[0]));
            h.keys("p");
            let display = h.frame(1440., 900.).windows.remove(0).display;
            let (line, column) = display.cursor.unwrap();
            assert_eq!((display.lines[line].number, column), (5, 2));
            assert!(
                display.lines[line]
                    .decorations
                    .contains(&Decoration::Background(FaceName::CompletionSelected))
            );
            assert!(
                display
                    .lines
                    .iter()
                    .flat_map(|l| &l.spans)
                    .any(|s| s.text == "* task"
                        && s.face.fg == theme.face(FaceName::WorkspaceCurrent).fg
                        && s.face.bold)
            );
            assert_eq!(h.text(), snapshot);
            let buffer = h.core.buffers.get_mut(dirty).unwrap();
            buffer.read_only = true;
            buffer.conflict = Some(0);
            h.keys("C-x t l");
            assert_eq!(
                contents(&h)[10],
                "dirty.rs src/ ● unsaved ⊘ read-only [conflict]"
            );
            let display = h.frame(1440., 900.).windows.remove(0).display;
            let row = display.lines.iter().find(|l| l.number == 10).unwrap();
            for (text, face) in [
                ("● unsaved", FaceName::StatusModified),
                ("⊘ read-only", FaceName::StatusFailed),
                ("[conflict]", FaceName::StatusFailed),
            ] {
                assert!(
                    row.spans
                        .iter()
                        .any(|s| s.text == text && s.face.fg == theme.face(face).fg)
                );
            }
            let buffer = h.core.buffers.get_mut(dirty).unwrap();
            buffer.read_only = false;
            buffer.conflict = None;
            buffer.set_saved();
            h.keys("C-x t l");
            assert_eq!(contents(&h)[6], "* task 8 buffers");
        }
    }

    #[test]
    fn file_locations_use_the_owning_projects_roots_and_escape_controls() {
        let mut h = Harness::new(&[]);
        let first = h
            .core
            .add_project(Loaded::ad_hoc("first\nproject", &h.path("first")));
        let workspace = h.core.new_workspace("one", first);
        h.core.switch_workspace(workspace).unwrap();
        h.open("first/src/a.rs");
        let second = h
            .core
            .new_workspace("two", h.core.current_workspace().project);
        h.core.switch_workspace(second).unwrap();
        h.open("first/src/a.rs");
        h.core
            .projects
            .iter_mut()
            .find(|p| p.id == first)
            .unwrap()
            .project
            .primary
            .path = h.path("first");
        h.core.switch_workspace(workspace).unwrap();
        h.new_workspace("three", "q");
        h.keys("C-x t l");
        let lines = contents(&h);
        assert_eq!(lines.iter().filter(|l| l.ends_with(" src/")).count(), 2);
        assert!(
            lines
                .iter()
                .any(|l| l == "first\\nproject /aiditor-test/first")
        );
    }

    #[test]
    fn tree_groups_workspaces_and_only_their_listed_buffers() {
        let mut h = workspaces();
        h.core.buffers.create(" hidden");
        h.keys("C-g C-x t l");
        assert_eq!(
            contents(&h),
            [
                "Workspaces — RET switch, n/p move, q quit",
                "",
                "p /aiditor-test",
                "a 1 buffer",
                "*scratch*",
                "* b 2 buffers",
                "*scratch*",
                "notes.txt",
                "q /aiditor-test",
                "c 1 buffer",
                "*scratch*"
            ]
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
            contents(&h),
            [
                "Workspaces — RET switch, n/p move, q quit",
                "",
                "app /aiditor-test/first",
                "a\\nnext 1 buffer",
                "*scratch*",
                "app /aiditor-test/second",
                "* b 2 buffers",
                "*scratch*",
                "tab\\tname"
            ]
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
        let rows = contents(&h);
        assert_eq!(
            &rows[rows.len() - 2..],
            [
                "alpha /aiditor-test/alpha · no workspaces",
                "zeta /aiditor-test/zeta · no workspaces"
            ]
        );
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
                assert_eq!(
                    h.core.current().line_col(h.point()).unwrap().1,
                    target.column()
                );
                h.keys(next);
            }
            assert_eq!(row(&h), *expected.last().unwrap());
            for target in expected.iter().rev() {
                assert_eq!(row(&h), *target);
                assert_eq!(
                    h.core.current().line_col(h.point()).unwrap().1,
                    target.column()
                );
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
        assert_eq!(&contents(&h)[7..9], ["added.txt", "notes.txt"]);
        let notes = h.core.buffers.find_here("notes.txt").unwrap();
        h.core.kill_buffer(notes);
        h.core.rename_workspace("renamed").unwrap();
        h.keys("C-x t l");
        assert_eq!(
            &contents(&h)[5..9],
            [
                "* renamed 2 buffers",
                "*scratch*",
                "added.txt",
                "q /aiditor-test"
            ]
        );
        h.core.kill_workspace(b).unwrap();
        assert_eq!(h.core.windows.window.buffer(), other_list);
        h.keys("C-x t l q C-x t l");
        assert_eq!(h.core.current().name, "*Workspaces*");
        assert_eq!(h.core.current().workspace_rows.len(), 4);
    }
}
