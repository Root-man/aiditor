//! A temporary tree of projects, workspaces and their buffers.

use crate::{
    Core, Ctx,
    buffer::{Buffer, BufferId, Decoration, FaceRef, Kind, Prop},
    display::{ModeLine, WindowDisplay},
    project::ProjectId,
    settings::FaceName,
    workspace::WorkspaceId,
};
use std::path::PathBuf;

pub(crate) const WIDE_COLUMNS: usize = 20 + 1 + 46 + 3 + 26;

struct Entry {
    row: Option<Row>,
    folded: bool,
    children: Vec<Entry>,
    name: String,
    column: usize,
    face: Vec<FaceRef>,
    location: crate::display::Segment,
    buffer: Option<BufferId>,
    state: Vec<(String, FaceName)>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Target {
    Row(Row),
    Buffer(BufferId),
}

impl Entry {
    fn target(&self) -> Option<Target> {
        self.row
            .map(Target::Row)
            .or_else(|| self.buffer.map(Target::Buffer))
    }
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
    fn labels(&self) -> (String, crate::display::Segment, &[(String, FaceName)]) {
        let mut name = self.name.clone();
        if !self.children.is_empty() {
            let marker = if matches!(self.row, Some(Row::Project(_))) {
                0
            } else {
                2
            };
            name.replace_range(marker..marker + 1, if self.folded { "▸" } else { "▾" });
        }
        let project = matches!(self.row, Some(Row::Project(_)));
        let mut location: crate::display::Segment = self
            .location
            .iter()
            .map(|(text, face)| (text.escape_debug().to_string(), *face))
            .collect();
        let summary = if project && self.folded && !self.children.is_empty() {
            format!(
                " · {} workspace{}",
                self.children.len(),
                if self.children.len() == 1 { "" } else { "s" }
            )
        } else if project && self.children.is_empty() {
            " · no workspaces".into()
        } else {
            String::new()
        };
        if !summary.is_empty() {
            location.push((summary, FaceName::Annotation));
        }
        let state = if project && !self.folded {
            &[][..]
        } else {
            &self.state
        };

        (name, location, state)
    }

    fn fit(&self, cols: usize, theme: &crate::settings::Theme) -> Vec<crate::display::Span> {
        use crate::display::{Span, cut, share_room, shorten};
        let (name, location, state) = self.labels();
        let span = |text: String, face| Span {
            text,
            face: theme.face(face),
            fold: false,
        };
        let narrow = cols < WIDE_COLUMNS;
        let mut status = Vec::new();
        for (text, face) in state {
            let text = if narrow {
                match text.chars().next() {
                    Some('●' | '⊘' | '◉') => text.chars().take(1).collect(),
                    Some('[') => "⊘".into(),
                    Some('e') => "●".into(),
                    _ => text.clone(),
                }
            } else {
                text.clone()
            };
            status.push(span(text, *face));
        }
        let state_len: usize = status.iter().map(|s| s.text.chars().count()).sum();
        let project = matches!(self.row, Some(Row::Project(_)));
        let location = if narrow {
            if project {
                self.location
                    .iter()
                    .map(|(s, f)| (s.escape_debug().to_string(), *f))
                    .collect()
            } else {
                Vec::new()
            }
        } else {
            location
        };
        let state_width = if narrow {
            2.max(state_len)
        } else {
            26.max(state_len)
        };
        let gap = if state_width > 0 {
            if narrow { 2 } else { 3 }
        } else {
            0
        };
        let state_width = state_width.min(cols);
        let room = cols.saturating_sub(state_width + gap);
        let name_len = name.chars().count();
        let location_len: usize = location.iter().map(|(s, _)| s.chars().count()).sum();
        let location_width = if narrow {
            location_len
        } else {
            46.max(location_len)
        };
        let location_gap = usize::from(location_width > 0) * if narrow { 2 } else { 1 };
        let (name_room, location_room) =
            share_room(name_len, location_width, room.saturating_sub(location_gap));
        let mut name_spans = vec![span(
            name.chars().take(self.column).collect(),
            FaceName::SessionThought,
        )];
        let face_start = if self
            .face
            .contains(&FaceRef::Face(FaceName::WorkspaceCurrent))
        {
            4
        } else {
            self.column
        };
        if face_start < self.column {
            name_spans[0].text = name.chars().take(face_start).collect();
        }
        name_spans.push(Span {
            text: name.chars().skip(face_start).collect(),
            face: theme.resolve(&self.face),
            fold: false,
        });
        let mut out = cut(name_spans, name_room);
        let name_width: usize = out.iter().map(|s| s.text.chars().count()).sum();
        if location_room > 0 {
            out.push(span(
                " ".repeat(room.saturating_sub(name_width + location_room)),
                FaceName::Annotation,
            ));
            let mut skip = location_len.saturating_sub(location_room);
            let mut shortened = skip == 0;
            for (text, face) in location {
                let len = text.chars().count();
                let text = if shortened {
                    text
                } else if skip >= len {
                    skip -= len;
                    continue;
                } else {
                    shortened = true;
                    shorten(&text, skip)
                };
                out.push(span(text, face));
            }
            out.push(span(
                " ".repeat(location_room.saturating_sub(location_len)),
                FaceName::Annotation,
            ));
        }
        if state_width > 0 {
            let used: usize = out.iter().map(|s| s.text.chars().count()).sum();
            out.push(span(
                " ".repeat(cols.saturating_sub(state_width + used)),
                FaceName::Annotation,
            ));
            let padding = state_width.saturating_sub(state_len);
            out.push(span(" ".repeat(padding), FaceName::Annotation));
            out.extend(cut(status, state_width));
        }
        cut(out, cols)
    }

    fn render(&self, buffer: &mut Buffer) {
        let (name, location, state) = self.labels();
        let start = buffer.len_chars() + usize::from(buffer.len_chars() > 0);
        let name_len = name.chars().count();
        let location_text: String = location.iter().map(|(s, _)| s.as_str()).collect();
        let state_len: usize = state.iter().map(|(s, _)| s.chars().count()).sum();
        let mut line = format!(
            "{} {}{}   {}",
            name,
            location_text,
            " ".repeat(46usize.saturating_sub(location_text.chars().count())),
            " ".repeat(26usize.saturating_sub(state_len))
        );
        let state_at = start + line.chars().count();
        for (text, _) in state {
            line.push_str(text);
        }
        let separator = if buffer.len_chars() == 0 { "" } else { "\n" };
        buffer.insert_output(buffer.len_chars(), &format!("{separator}{line}"));
        let mut named = |range, refs| {
            buffer
                .add_property(range, Prop::Named(refs))
                .expect("just inserted")
        };
        named(
            start..start + self.column,
            vec![FaceRef::Face(FaceName::SessionThought)],
        );
        let face_start = if self
            .face
            .contains(&FaceRef::Face(FaceName::WorkspaceCurrent))
        {
            4
        } else {
            self.column
        };
        named(start + face_start..start + name_len, self.face.clone());
        let mut at = start + name_len + 1;
        for (text, face) in &location {
            let end = at + text.chars().count();
            named(at..end, vec![FaceRef::Face(*face)]);
            at = end;
        }
        named(at..state_at, vec![FaceRef::Face(FaceName::Annotation)]);
        let mut at = state_at;
        for (text, face) in state {
            let end = at + text.chars().count();
            buffer
                .add_property(at..end, Prop::Named(vec![FaceRef::Face(*face)]))
                .expect("just inserted");
            at = end;
        }
        buffer
            .add_property(start + name_len..start + name_len + 1, Prop::AlignRight)
            .expect("just inserted");
    }
}

pub(crate) struct Snapshot {
    entries: Vec<Entry>,
}

impl Snapshot {
    pub(crate) fn counts(&self) -> (usize, usize) {
        (
            self.entries.len(),
            self.entries.iter().map(|p| p.children.len()).sum(),
        )
    }

    fn entries(&self) -> impl Iterator<Item = &Entry> {
        self.entries
            .iter()
            .flat_map(|p| std::iter::once(p).chain(p.children.iter()))
    }

    fn visible(&self) -> Vec<Option<&Entry>> {
        fn visit<'a>(entry: &'a Entry, rows: &mut Vec<Option<&'a Entry>>) {
            rows.push(Some(entry));
            if !entry.folded {
                for child in &entry.children {
                    visit(child, rows);
                }
            }
        }
        let mut rows = Vec::new();
        for (i, entry) in self.entries.iter().enumerate() {
            if i > 0 {
                rows.push(None);
            }
            visit(entry, &mut rows);
        }
        rows
    }

    fn selection(&self, buffer: &Buffer, point: usize) -> (Vec<Target>, usize) {
        let (line, col) = buffer.line_col(point).unwrap_or_default();
        let visible = self.visible();
        let selected = visible
            .iter()
            .take(line + 1)
            .rev()
            .flatten()
            .next()
            .and_then(|e| e.target());
        for project in &self.entries {
            let parent = project.target().expect("project identity");
            if selected == Some(parent) {
                return (vec![parent], col);
            }
            for workspace in &project.children {
                let owner = workspace.target().expect("workspace identity");
                if selected == Some(owner) {
                    return (vec![owner, parent], col);
                }
                for buffer in &workspace.children {
                    let target = buffer.target().expect("buffer identity");
                    if selected == Some(target) {
                        return (vec![target, owner, parent], col);
                    }
                }
            }
        }
        (Vec::new(), col)
    }

    fn point(&self, buffer: &Buffer, targets: &[Target], col: usize) -> Option<usize> {
        let visible = self.visible();
        for (i, target) in targets.iter().enumerate() {
            if let Some((line, Some(entry))) = visible
                .iter()
                .enumerate()
                .find(|(_, e)| e.is_some_and(|e| e.target() == Some(*target)))
            {
                return buffer
                    .char_idx(line, if i == 0 { col } else { entry.column })
                    .ok();
            }
        }
        None
    }

    /// Puts the selected row's background and the list's mode line, with point on `line` (from
    /// 1), on `buffer`'s `display`.
    pub(crate) fn decorate(&self, buffer: &Buffer, display: &mut WindowDisplay, line: usize) {
        let lines = &mut display.lines;
        if let Some(number) = (display.cursor)
            .map(|(l, _)| lines[l].number)
            .filter(|n| buffer.workspace_rows().iter().any(|(line, _)| line == n))
        {
            for line in lines.iter_mut().filter(|l| l.number == number) {
                (line.decorations).push(Decoration::Background(FaceName::CompletionSelected));
            }
        }
        let segment = |text: &str, face| vec![(text.to_string(), face)];
        let (projects, workspaces) = self.counts();
        let rows = buffer.workspace_rows().len();
        let position = buffer
            .workspace_rows()
            .partition_point(|(row, _)| *row < line)
            .max(1);
        display.mode_line = ModeLine {
            left: vec![
                segment(&buffer.name, FaceName::ModeLine),
                segment(
                    &format!("{projects} projects, {workspaces} workspaces"),
                    FaceName::ModeLineDim,
                ),
            ],
            // An empty list has no position.
            right: (rows > 0)
                .then(|| segment(&format!("{position}/{rows}"), FaceName::ModeLine))
                .into_iter()
                .collect(),
            path: None,
        };
    }

    pub(crate) fn fit(
        &self,
        lines: &mut [crate::display::Line],
        cols: usize,
        theme: &crate::settings::Theme,
    ) {
        if cols == usize::MAX {
            return;
        }
        let rows = self.visible();
        for line in lines {
            if let Some(Some(entry)) = rows.get(line.number) {
                line.spans = entry.fit(cols, theme);
                line.aligned = true;
            }
        }
    }

    /// Renders it into `buffer`, which then holds it.
    fn install(self, buffer: &mut Buffer) {
        buffer.remove_properties(0..buffer.len_chars());
        buffer.set_text("");
        let mut rows = Vec::new();
        if self.entries.is_empty() {
            buffer.insert_output(0, "No projects");
            let refs = vec![FaceRef::Face(FaceName::Annotation)];
            (buffer.add_property(0..11, Prop::Named(refs))).expect("just inserted");
        }
        for (line, entry) in self.visible().into_iter().enumerate() {
            if let Some(entry) = entry {
                entry.render(buffer);
                if let Some(row) = entry.row {
                    rows.push((line, row));
                }
            } else {
                buffer.insert_output(buffer.len_chars(), "\n ");
                let end = buffer.len_chars();
                buffer
                    .add_property(
                        end - 1..end,
                        Prop::Line(Decoration::HRule(FaceName::Separator)),
                    )
                    .expect("separator row exists");
            }
        }
        buffer.set_saved();
        buffer.kind = Kind::WorkspaceList {
            snapshot: self,
            rows,
        };
    }
}

fn details(
    core: &Core,
    id: crate::buffer::BufferId,
    buffer: &Buffer,
    roots: &[PathBuf],
) -> (crate::display::Segment, crate::display::Segment) {
    if let Some((_, session)) = core.session_of(id) {
        let location = session.model_effort();
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
        return (vec![(location, FaceName::Annotation)], state);
    }
    let location = buffer
        .path()
        .map(|file| {
            crate::display::file_mode_line(buffer, file, roots, None, (1, 0), "All").left[0]
                .iter()
                .filter(|(_, f)| *f == FaceName::ModeLineDim)
                .map(|(s, _)| s.as_str())
                .collect()
        })
        .unwrap_or_default();
    let deleted = buffer.file.as_ref().is_some_and(|f| f.deleted);
    let mut state = Vec::new();
    for (shown, text, face) in [
        (buffer.modified(), "● unsaved", FaceName::StatusModified),
        (buffer.read_only, "⊘ read-only", FaceName::StatusFailed),
        (deleted, "[deleted]", FaceName::StatusFailed),
        (
            !deleted && buffer.file.as_ref().is_some_and(|f| f.conflict.is_some()),
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
    (vec![(location, FaceName::Annotation)], state)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Row {
    Project(ProjectId),
    Workspace(WorkspaceId),
}

/// The echo line's actions: the commands whose keys a hint shows, `/`-joined, and its label.
pub(crate) const HINTS: &[(&[&str], &str)] = &[
    (&["workspace-list-next", "workspace-list-previous"], "move"),
    (&["workspace-list-visit"], "switch"),
    (&["workspace-list-toggle"], "fold"),
    (&["workspace-list"], "refresh"),
    (&["workspace-list-quit"], "quit"),
];

/// The lobby's: `q` has nothing to go back to, so the ways out of the lobby take its place.
pub(crate) const LOBBY_HINTS: &[(&[&str], &str)] = &[
    (&["workspace-list-next", "workspace-list-previous"], "move"),
    (&["workspace-list-visit"], "switch"),
    (&["workspace-list-toggle"], "fold"),
    (&["workspace-list"], "refresh"),
    (&["find-file"], "find file"),
    (&["project-switch"], "project"),
];

impl Core {
    /// `hints` with the keys that run their commands now, leaving out an action none is bound to.
    pub(crate) fn key_hints(
        &self,
        hints: &[(&[&str], &'static str)],
    ) -> Vec<(String, &'static str)> {
        let keys = |command| {
            let keys = self.where_is(command)?;
            Some(
                keys.iter()
                    .map(|k| k.to_string())
                    .collect::<Vec<_>>()
                    .join(" "),
            )
        };
        (hints.iter())
            .filter_map(|(commands, label)| {
                let keys: Vec<_> = commands.iter().filter_map(|c| keys(c)).collect();
                (!keys.is_empty()).then(|| (keys.join("/"), *label))
            })
            .collect()
    }
}

pub(crate) const MODE: &str = "workspace-list-mode";

impl Ctx<'_> {
    pub(crate) fn list_workspaces(&mut self) -> Result<(), String> {
        self.outside_minibuffer("Can't list workspaces in the minibuffer")?;
        self.core.list_workspaces()
    }
}

impl Core {
    /// Opens or refreshes the current workspace's list, or the lobby's, in the selected window.
    pub(crate) fn list_workspaces(&mut self) -> Result<(), String> {
        let core = self;
        let current = core.workspace();
        let found = core
            .buffers
            .recent()
            .find(|(_, b)| b.major_mode == MODE && b.workspace == current)
            .map(|(id, _)| id);
        let selections: Vec<_> = found
            .map(|id| {
                let buffer = core.buffers.get(id).unwrap();
                let Some(snapshot) = buffer.workspace_list() else {
                    return Vec::new();
                };
                std::iter::once((core.windows.selected, &core.windows.window))
                    .chain(core.windows.others.iter().map(|(&id, w)| (id, w)))
                    .filter(|(_, w)| w.buffer() == id)
                    .map(|(id, w)| (id, snapshot.selection(buffer, w.point(&core.buffers))))
                    .collect()
            })
            .unwrap_or_default();
        let old = found.and_then(|id| core.buffers.get(id).unwrap().workspace_list());
        let folded = |row, default| {
            old.and_then(|snapshot| snapshot.entries().find(|e| e.row == Some(row)))
                .map_or(default, |e| e.folded)
        };
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
        for project_id in &projects {
            let project = &core.projects[core.project_at(*project_id)?].project;
            let roots: Vec<_> = project.all_roots().map(|r| r.path.clone()).collect();
            let workspaces: Vec<_> = core
                .workspaces
                .iter()
                .filter(|w| w.project == *project_id)
                .collect();

            let mut project_entry = Entry {
                row: Some(Row::Project(*project_id)),
                folded: folded(Row::Project(*project_id), false),
                children: Vec::new(),
                name: format!("  {}", project.name.escape_debug()),
                column: 2,
                face: vec![FaceRef::Bold],
                location: vec![(
                    project.primary.path.to_string_lossy().into_owned(),
                    FaceName::Annotation,
                )],
                buffer: None,
                state: workspaces
                    .iter()
                    .filter_map(|w| core.workspace_activity(w.id))
                    .max()
                    .map(|a| (a.text().to_string(), a.face()))
                    .into_iter()
                    .collect(),
            };
            for workspace in workspaces {
                let mut buffers: Vec<_> = core
                    .buffers
                    .recent()
                    .filter(|(id, b)| {
                        b.workspace == Some(workspace.id)
                            && (b.file.is_some()
                                || b.terminal().is_some()
                                || core.session_of(*id).is_some())
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
                let current = Some(workspace.id) == current;
                let mut workspace_entry = Entry {
                    row: Some(Row::Workspace(workspace.id)),
                    folded: folded(Row::Workspace(workspace.id), !current),
                    children: Vec::new(),
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
                    location: vec![(location, FaceName::Annotation)],
                    buffer: None,
                    state: activity
                        .map(|a| (a.text().to_string(), a.face()))
                        .into_iter()
                        .collect(),
                };
                let suffix = format!("<{}>", workspace.name);
                for (id, buffer) in buffers {
                    let name = buffer.name.strip_suffix(&suffix).unwrap_or(&buffer.name);
                    let (location, state) = details(core, id, buffer, &roots);
                    workspace_entry.children.push(Entry {
                        row: None,
                        folded: false,
                        children: Vec::new(),
                        name: format!("      {}", name.escape_debug()),
                        column: 6,
                        face: vec![FaceRef::Face(FaceName::ModeLineDim)],
                        location,
                        buffer: Some(id),
                        state,
                    });
                }
                project_entry.children.push(workspace_entry);
            }
            entries.push(project_entry);
        }
        let id = found.unwrap_or_else(|| core.buffers.create("*Workspaces*"));
        let buffer = core.buffers.get_mut(id).expect("list buffer is live");
        buffer.major_mode = MODE;
        buffer.read_only = true;
        Snapshot { entries }.install(buffer);
        let rows = buffer.workspace_rows();
        let target = match current {
            Some(c) if rows.iter().any(|(_, r)| *r == Row::Workspace(c)) => Some(Row::Workspace(c)),
            Some(c) => Some(Row::Project(
                core.workspaces.iter().find(|w| w.id == c).unwrap().project,
            )),
            // The lobby starts on its first project, if any.
            None => rows.first().map(|(_, r)| *r),
        };
        let point = match target {
            Some(target) => {
                let line = (rows.iter().find(|(_, row)| *row == target))
                    .expect("current workspace or its project is listed")
                    .0;
                buffer.char_idx(line, target.column())?
            }
            None => 0,
        };
        let selected_preserved = selections
            .iter()
            .any(|(id, _)| *id == core.windows.selected);
        let snapshot = buffer.workspace_list().expect("list has a snapshot");
        let points: Vec<_> = selections
            .into_iter()
            .map(|(id, (targets, col))| {
                (id, snapshot.point(buffer, &targets, col).unwrap_or(point))
            })
            .collect();
        core.switch_to(id);
        for (id, point) in points {
            let window = if id == core.windows.selected {
                &core.windows.window
            } else {
                core.windows.others.get(&id).expect("sharing window exists")
            };
            window.set_point(&mut core.buffers, point)?;
        }
        if !selected_preserved {
            core.windows.window.set_point(&mut core.buffers, point)?;
        }
        Ok(())
    }
}

impl Ctx<'_> {
    fn workspace_list_rows(&self) -> Result<&[(usize, Row)], String> {
        let buffer = self.core.current();
        if buffer.major_mode != MODE || buffer.workspace_list().is_none() {
            return Err("Not in a workspace list".into());
        }
        Ok(buffer.workspace_rows())
    }

    pub(crate) fn workspace_list_move(&mut self, forward: bool) -> Result<(), String> {
        let rows = self.workspace_list_rows()?;
        let line = self.core.current().line_col(self.core.point())?.0;
        // An empty lobby list has nowhere to go.
        let (Some(first), Some(last)) = (rows.first(), rows.last()) else {
            return Ok(());
        };
        let target = if forward {
            rows.iter().find(|(l, _)| *l > line).unwrap_or(last)
        } else {
            rows.iter().rev().find(|(l, _)| *l < line).unwrap_or(first)
        };
        self.goto(self.core.current().char_idx(target.0, target.1.column())?)
    }

    pub(crate) fn workspace_list_toggle(&mut self) -> Result<(), String> {
        let rows = self.workspace_list_rows()?;
        let line = self.core.current().line_col(self.core.point())?.0;
        let Some((_, row)) = rows.iter().find(|(l, _)| *l == line) else {
            return Ok(());
        };
        let row = *row;
        let buffer = self.core.current_mut();
        let Kind::WorkspaceList { snapshot, .. } = &mut buffer.kind else {
            unreachable!("checked by workspace_list_rows");
        };
        let entry = snapshot.entries.iter_mut().find(|e| e.row == Some(row));
        let entry = match entry {
            Some(entry) => Some(entry),
            None => snapshot
                .entries
                .iter_mut()
                .flat_map(|p| &mut p.children)
                .find(|e| e.row == Some(row)),
        }
        .expect("visible row belongs to snapshot");
        if entry.children.is_empty() {
            return Ok(());
        }
        entry.folded = !entry.folded;
        let Kind::WorkspaceList { snapshot, .. } = std::mem::replace(&mut buffer.kind, Kind::Text)
        else {
            unreachable!("checked above");
        };
        snapshot.install(buffer);
        let line = buffer
            .workspace_rows()
            .iter()
            .find(|(_, r)| *r == row)
            .unwrap()
            .0;
        let point = buffer.char_idx(line, row.column())?;
        self.goto(point)
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
        // The lobby has nothing to go back to.
        if self.core.workspace().is_none() {
            return Ok(());
        }
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
        let launch = h.core.workspace().unwrap();
        h.new_workspace("a", "p");
        h.open("a.txt");
        h.new_workspace("b", "p");
        h.open("notes.txt");
        h.new_workspace("c", "q");
        h.open("c.txt");
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
            .workspace_rows()
            .iter()
            .find(|(l, _)| *l == line)
            .unwrap()
            .1
    }

    fn select(h: &mut Harness, target: Row) {
        let line = h
            .core
            .current()
            .workspace_rows()
            .iter()
            .find(|(_, row)| *row == target)
            .unwrap()
            .0;
        let at = h.core.current().char_idx(line, target.column()).unwrap();
        h.core
            .windows
            .window
            .set_point(&mut h.core.buffers, at)
            .unwrap();
    }

    fn drawn(spans: &[crate::display::Span]) -> String {
        spans.iter().map(|s| s.text.as_str()).collect()
    }

    #[test]
    fn rows_fit_names_roots_and_state_faces_at_several_widths() {
        let h = workspaces();
        let theme = &h.core.settings.theme;
        let mut entry = Entry {
            row: Some(Row::Project(ProjectId(0))),
            folded: true,
            children: Vec::new(),
            name: "  long-project-name-éééééééééééééééé".into(),
            column: 2,
            face: vec![FaceRef::Bold],
            location: vec![(
                "/a/very/long/primary/root/project".into(),
                FaceName::Annotation,
            )],
            buffer: None,
            state: vec![("● awaiting permission".into(), FaceName::StatusWaiting)],
        };
        let narrow = entry.fit(48, theme);
        assert!(narrow.iter().any(|s| s.text.ends_with('…') && s.face.bold));
        assert!(
            narrow
                .iter()
                .any(|s| s.text.starts_with('…') && s.text.ends_with("project"))
        );
        assert!(
            narrow
                .iter()
                .any(|s| s.text == "●" && s.face == theme.face(FaceName::StatusWaiting))
        );
        assert!(drawn(&entry.fit(120, theme)).contains("● awaiting permission"));
        for cols in 0..150 {
            assert!(
                drawn(&entry.fit(cols, theme)).chars().count() <= cols,
                "{cols}"
            );
        }
        entry.row = None;
        entry.state = vec![
            ("● unsaved".into(), FaceName::StatusModified),
            (" ".into(), FaceName::Annotation),
            ("⊘ read-only".into(), FaceName::StatusFailed),
            (" ".into(), FaceName::Annotation),
            ("[conflict]".into(), FaceName::StatusFailed),
        ];
        assert!(drawn(&entry.fit(48, theme)).ends_with("● ⊘ ⊘"));
        entry.state = vec![("exit 101".into(), FaceName::StatusFailed)];
        assert!(
            entry
                .fit(48, theme)
                .iter()
                .any(|s| s.text == "●" && s.face == theme.face(FaceName::StatusFailed))
        );
    }

    #[test]
    fn workspace_list_entries_stay_on_one_visual_row_before_fitting_and_scrolling() {
        let mut h = workspaces();
        let project = h.core.current_workspace().unwrap().project;
        h.core
            .projects
            .iter_mut()
            .find(|p| p.id == project)
            .unwrap()
            .project
            .primary
            .path = PathBuf::from(format!("/{}", "long-project-root/".repeat(12)));
        h.core
            .workspaces
            .iter_mut()
            .find(|w| w.project == project)
            .unwrap()
            .name = "long-workspace-name-".repeat(8);
        h.open(&format!("{}.rs", "long-file-name-".repeat(8)));
        let (terminal, _) = h.core.create_terminal("long-terminal-name", 2, 10);
        h.send(crate::Msg::TerminalOutput {
            id: terminal,
            bytes: format!("\x1b]133;C;cmdline_url={}\x07", "long-command-".repeat(20))
                .into_bytes(),
        });
        h.keys("C-x t l C-x 3 C-x 3");
        let snapshot = h.text();
        let expected: Vec<_> = (0..h.core.current().len_lines()).collect();
        let point_line = h.core.current().line_col(h.point()).unwrap().0;
        for font_size in [12., 20., 32.] {
            h.core.settings.font_size = font_size;
            for width in [900., 1200., 1800.] {
                for _ in 0..2 {
                    h.send(crate::Msg::Resize {
                        width,
                        height: 2000.,
                        cell_width: font_size * 0.6,
                    });
                    for window in h.core.frame().windows {
                        let numbers: Vec<_> =
                            window.display.lines.iter().map(|l| l.number).collect();
                        assert_eq!(
                            numbers, expected,
                            "font {font_size}, frame {width}, window {}",
                            window.rect.width
                        );
                        let (row, _) = window.display.cursor.expect("point is visible");
                        assert_eq!(window.display.lines[row].number, point_line);
                    }
                    h.keys("M-x toggle-truncate-lines RET");
                }
            }
        }
        assert_eq!(h.text(), snapshot);
        h.keys("M-x next-line RET");
        assert_eq!(
            h.core.current().line_col(h.point()).unwrap().0,
            point_line + 1
        );
        h.keys("n n");
        let selected = row(&h);
        for width in [900., 1200., 900.] {
            h.send(crate::Msg::Resize {
                width,
                height: 140.,
                cell_width: 19.2,
            });
            for window in h.core.frame().windows {
                let numbers: Vec<_> = window.display.lines.iter().map(|l| l.number).collect();
                assert_eq!(
                    numbers,
                    (numbers[0]..numbers[0] + numbers.len()).collect::<Vec<_>>()
                );
                let (at, _) = window
                    .display
                    .cursor
                    .expect("point stays within the viewport");
                assert!(at < h.core.settings.text_rows(window.rect.height));
            }
            assert_eq!(row(&h), selected);
            assert_eq!(h.core.windows.window.top_row(), 0);
        }
    }

    #[test]
    fn fitted_location_cuts_keep_the_retained_segment_faces() {
        let h = workspaces();
        let theme = &h.core.settings.theme;
        let entry = Entry {
            row: None,
            folded: false,
            children: Vec::new(),
            name: "      session".into(),
            column: 6,
            face: vec![FaceRef::Face(FaceName::ModeLineDim)],
            buffer: None,
            location: vec![
                ("very-long-model-name-".repeat(4), FaceName::QualityEpic),
                (" · ".into(), FaceName::ModeLineDim),
                ("High".into(), FaceName::QualityRare),
            ],
            state: Vec::new(),
        };
        let spans = entry.fit(WIDE_COLUMNS, theme);
        assert!(
            spans
                .iter()
                .any(|s| s.text.starts_with('…') && s.face == theme.face(FaceName::QualityEpic))
        );
        assert!(
            spans
                .iter()
                .any(|s| s.text == "High" && s.face == theme.face(FaceName::QualityRare))
        );
        assert_eq!(drawn(&spans).chars().count(), WIDE_COLUMNS);
    }

    #[test]
    fn resize_refits_the_same_snapshot_and_unequal_windows_independently() {
        let mut h = workspaces();
        let current = h.core.workspace().unwrap();
        h.core
            .workspaces
            .iter_mut()
            .find(|w| w.id == current)
            .unwrap()
            .name = "invalid-site-redirects-in-the-user-graphql-server".into();
        h.keys("C-x t l TAB");
        let text = h.text();
        let point = h.point();
        let rows = h.core.current().workspace_rows().to_vec();
        let list = h.core.windows.window.buffer();
        h.open("later");
        h.core.switch_to(list);
        let wide = h.frame(1440., 900.).windows.remove(0);
        assert!(drawn(&wide.mode_line.left).contains("2 projects, 3 workspaces"));
        assert!(
            wide.display
                .lines
                .iter()
                .any(|l| drawn(&l.spans).contains("1 buffer"))
        );
        h.keys("C-x 3");
        let half = h.frame(900., 600.).windows.remove(0);
        let (line, col) = half.display.cursor.unwrap();
        assert_eq!(col, 6);
        assert!(
            drawn(&half.display.lines[line].spans)
                .trim_end()
                .ends_with('…')
        );
        assert_eq!(drawn(&half.mode_line.left), "*Workspaces*");
        assert_eq!(drawn(&half.mode_line.right), "3/5");
        assert!(half.display.lines[line].decorations.contains(
            &crate::buffer::Decoration::Background(FaceName::CompletionSelected)
        ));
        assert!(
            half.display.lines[line]
                .spans
                .iter()
                .any(|s| s.text.starts_with("* ") && s.face.bold)
        );
        h.keys("C-x 3");
        let unequal = h.frame(900., 600.);
        let names: Vec<_> = unequal
            .windows
            .iter()
            .map(|w| {
                let line = w
                    .display
                    .lines
                    .iter()
                    .find(|l| {
                        l.number
                            == rows
                                .iter()
                                .find(|(_, r)| *r == Row::Workspace(current))
                                .unwrap()
                                .0
                    })
                    .unwrap();
                drawn(&line.spans)
            })
            .collect();
        assert!(names[0].chars().count() < names[2].chars().count());
        h.keys("C-x 1");
        let restored = h.frame(1440., 900.).windows.remove(0);
        assert_eq!(restored.display.lines, wide.display.lines);
        assert_eq!(h.text(), text);
        assert_eq!(h.point(), point);
        assert_eq!(h.core.current().workspace_rows(), rows);
        assert_eq!(row(&h), Row::Workspace(current));
        h.keys("TAB");
        assert!(h.text().lines().any(|l| l.trim() == "notes.txt"));
        h.keys("RET");
        assert_eq!(h.core.workspace().unwrap(), current);
        assert_eq!(h.text(), "");
    }

    #[test]
    fn switching_from_the_list_leaves_the_split_as_it_was() {
        let mut h = workspaces();
        let a = h.core.workspaces[0].id;
        h.keys("C-x 3 C-x o");
        h.open("other.txt");
        h.keys("C-x o C-x t l");
        select(&mut h, Row::Workspace(a));
        h.keys("RET");
        h.core.switch_to_workspace("b").unwrap();
        let windows = std::iter::once(&h.core.windows.window).chain(h.core.windows.others.values());
        let mut shown: Vec<_> = windows
            .map(|w| h.core.buffers.get(w.buffer()).unwrap().name.as_str())
            .collect();
        shown.sort();
        assert_eq!(shown, ["notes.txt", "other.txt"]);
    }

    #[test]
    fn refresh_keeps_each_windows_row_identity_and_falls_back_to_visible_owners() {
        let mut h = workspaces();
        let current = h.core.workspace().unwrap();
        let a = h.core.workspaces[0].id;
        let project = h.core.current_workspace().unwrap().project;
        let notes = h.core.buffers.find_here("notes.txt").unwrap();
        h.keys("C-x t l");
        select(&mut h, Row::Project(project));
        h.keys("g");
        assert_eq!(row(&h), Row::Project(project));
        select(&mut h, Row::Workspace(a));
        let old_line = h.core.current().line_col(h.point()).unwrap().0;
        h.core.workspaces.swap(0, 2);
        h.keys("g");
        assert_eq!(row(&h), Row::Workspace(a));
        assert!(h.core.current().line_col(h.point()).unwrap().0 > old_line);
        select(&mut h, Row::Workspace(current));
        h.keys("C-x 2");
        let buffer_line = h
            .core
            .current()
            .workspace_list()
            .unwrap()
            .visible()
            .iter()
            .position(|e| e.is_some_and(|e| e.buffer == Some(notes)))
            .unwrap();
        let point = h.core.current().char_idx(buffer_line, 8).unwrap();
        h.core
            .windows
            .window
            .set_point(&mut h.core.buffers, point)
            .unwrap();
        h.frame(1440., 140.);
        h.keys("C-x o");
        select(&mut h, Row::Workspace(a));
        let list = h.core.windows.window.buffer();
        h.open("before-notes.txt");
        h.core.switch_to(list);
        select(&mut h, Row::Workspace(a));
        h.keys("M-x workspace-list RET");
        assert_eq!(row(&h), Row::Workspace(a));
        h.keys("C-x o");
        let (line, col) = h.core.current().line_col(h.point()).unwrap();
        assert_eq!(col, 8);
        assert!(
            h.core
                .current()
                .text()
                .line(line)
                .to_string()
                .contains("notes.txt")
        );
        let before = h
            .frame(1440., 140.)
            .windows
            .iter()
            .find(|w| w.selected)
            .unwrap()
            .display
            .lines[0]
            .number;
        h.keys("g");
        let after = h
            .frame(1440., 140.)
            .windows
            .iter()
            .find(|w| w.selected)
            .unwrap()
            .display
            .lines[0]
            .number;
        assert_eq!(after, before);
        h.core.kill_buffer(notes);
        h.keys("g");
        assert_eq!(row(&h), Row::Workspace(current));
        assert_eq!(h.core.current().line_col(h.point()).unwrap().1, 6);
        h.keys("C-x o");
        assert_eq!(row(&h), Row::Workspace(a));
        h.core.kill_workspace(a).unwrap();
        h.keys("g");
        assert_eq!(row(&h), Row::Project(project));
        assert_eq!(h.core.current().line_col(h.point()).unwrap().1, 2);
    }

    #[test]
    fn horizontal_rules_separate_open_folded_and_empty_projects() {
        let mut h = workspaces();
        let empty = h
            .core
            .add_project(Loaded::ad_hoc("empty", &h.path("empty")));
        let project = h.core.current_workspace().unwrap().project;
        h.keys("C-x t l");
        for keys in ["", "p p TAB", "g", "TAB"] {
            h.keys(keys);
            for width in [1440., 450., 240.] {
                let frame = h.frame(width, 900.);
                let lines = &frame.windows[0].display.lines;
                let rules: Vec<_> = lines
                    .iter()
                    .enumerate()
                    .filter(|(_, l)| {
                        l.decorations
                            .contains(&Decoration::HRule(FaceName::Separator))
                    })
                    .collect();
                assert_eq!(rules.len(), 2);
                for (i, line) in rules {
                    assert!(i > 0 && i + 1 < lines.len());
                    assert_eq!(drawn(&line.spans), " ");
                    assert!(
                        h.core
                            .current()
                            .workspace_rows()
                            .iter()
                            .any(|(row, _)| *row == line.number + 1)
                    );
                }
            }
        }
        select(&mut h, Row::Project(project));
        h.keys("TAB n n");
        assert_eq!(row(&h), Row::Workspace(h.core.workspaces[2].id));
        h.keys("n");
        assert_eq!(row(&h), Row::Project(empty));
    }

    const LIST: &[(&str, &str)] = &[
        ("n/p", "move"),
        ("RET", "switch"),
        ("TAB", "fold"),
        ("g", "refresh"),
        ("q", "quit"),
    ];

    const LOBBY: &[(&str, &str)] = &[
        ("n/p", "move"),
        ("RET", "switch"),
        ("TAB", "fold"),
        ("g", "refresh"),
        ("C-x C-f", "find file"),
        ("C-x p p", "project"),
    ];

    fn shown(hints: &[(&str, &'static str)]) -> Vec<(String, &'static str)> {
        hints.iter().map(|&(k, a)| (k.into(), a)).collect()
    }

    #[test]
    fn hints_show_the_keys_bound_now() {
        let mut h = Harness::lobby(&[]);
        let keymap = concat!(
            "[global]\n\"C-x p p\" = \"beginning-of-buffer\"\n\"C-c p p\" = \"project-switch\"\n",
            "\"C-x C-f\" = \"beginning-of-buffer\"\n",
            "[workspace-list-mode]\n\"p\" = \"beginning-of-buffer\"\n\"g\" = \"beginning-of-buffer\"\n",
        );
        h.send(crate::Msg::KeymapLoaded(Ok(Some(keymap.into()))));
        let hints = h.core.frame().key_hints;
        // `p` falls back to `<up>`, `g` to the global `C-x t l`, and `find-file` has no key left.
        assert_eq!(
            hints,
            shown(&[
                ("n/<up>", "move"),
                ("RET", "switch"),
                ("TAB", "fold"),
                ("C-x t l", "refresh"),
                ("C-c p p", "project"),
            ])
        );
    }

    #[test]
    fn echo_hints_follow_selection_and_yield_to_messages_and_prompts() {
        use std::time::Duration;
        let mut h = workspaces();
        h.keys("C-x 3 C-x t l");
        assert_eq!(h.core.frame().key_hints, shown(LIST));
        assert!(h.text().starts_with("▾ p "));
        h.keys("C-x o");
        assert!(h.core.frame().key_hints.is_empty());
        h.keys("C-x o");
        assert_eq!(h.core.frame().key_hints, shown(LIST));
        let at = h.core.current().char_idx(3, 0).unwrap();
        h.core
            .windows
            .window
            .set_point(&mut h.core.buffers, at)
            .unwrap();
        h.keys("RET");
        assert_eq!(
            h.core.frame().message.as_deref(),
            Some("Select a project or workspace row")
        );
        assert!(h.core.frame().key_hints.is_empty());
        h.start_timer(Duration::from_millis(1), None, crate::Msg::Tick);
        h.advance(Duration::from_millis(2));
        assert_eq!(
            h.core.frame().message.as_deref(),
            Some("Select a project or workspace row")
        );
        h.keys("n");
        assert_eq!(h.core.frame().key_hints, shown(LIST));
        h.keys("M-x");
        let frame = h.core.frame();
        assert_eq!(frame.minibuffer.unwrap().prompt, "M-x ");
        assert!(frame.key_hints.is_empty());
        h.keys("workspace-list RET");
        assert_eq!(h.core.frame().key_hints, shown(LIST));
    }

    #[test]
    fn folds_keep_point_children_state_and_snapshot_until_refresh() {
        let mut h = workspaces();
        let current = h.core.workspace().unwrap();
        let project = h.core.current_workspace().unwrap().project;
        h.keys("C-x t l");
        let original = h.text();
        let point = h.point();
        h.keys("TAB");
        assert_eq!(row(&h), Row::Workspace(current));
        assert_eq!(h.point(), point);
        assert_eq!(contents(&h)[2], "▸ * b 1 buffer");
        let list = h.core.windows.window.buffer();
        h.open("later");
        h.core.switch_to(list);
        h.keys("TAB");
        assert_eq!(h.text(), original);
        h.keys("p TAB");
        assert_eq!(row(&h), Row::Workspace(h.core.workspaces[0].id));
        h.keys("p TAB");
        assert_eq!(row(&h), Row::Project(project));
        assert_eq!(h.core.current().line_col(h.point()).unwrap().1, 2);
        assert_eq!(
            &contents(&h)[0..3],
            ["▸ p /aiditor-test · 2 workspaces", "", "▾ q /aiditor-test"]
        );
        assert_eq!(
            h.frame(1440., 900.).windows[0].display.mode_line.text(),
            "*Workspaces* │ 2 projects, 3 workspaces  1/3"
        );
        h.keys("TAB n");
        assert_eq!(contents(&h)[1], "▾ a 1 buffer");
        h.keys("n TAB p p TAB C-x t l");
        assert_eq!(row(&h), Row::Project(project));
        assert_eq!(contents(&h)[0], "▸ p /aiditor-test · 2 workspaces");
        h.keys("TAB n");
        assert_eq!(contents(&h)[1], "▾ a 1 buffer");
        h.keys("n");
        assert_eq!(contents(&h)[3], "▸ * b 2 buffers");
        h.keys("q C-x t l");
        assert_eq!(row(&h), Row::Workspace(current));
        assert_eq!(contents(&h)[1], "▸ a 1 buffer");
        assert_eq!(contents(&h)[2], "▾ * b 2 buffers");
        assert!(contents(&h).iter().any(|s| s == "later"));
    }

    #[test]
    fn visible_navigation_and_activation_skip_folded_project_children() {
        for (next, previous) in [("n", "p"), ("C-n", "C-p"), ("<down>", "<up>")] {
            let mut h = workspaces();
            let project = h.core.current_workspace().unwrap().project;
            let q = h.core.workspaces[2].project;
            h.keys("C-x t l");
            select(&mut h, Row::Project(project));
            h.keys("TAB").keys(next);
            assert_eq!(row(&h), Row::Project(q));
            assert_eq!(
                h.frame(1440., 900.).windows[0].display.mode_line.text(),
                "*Workspaces* │ 2 projects, 3 workspaces  2/3"
            );
            h.keys(previous);
            assert_eq!(row(&h), Row::Project(project));
            h.keys(next).keys("RET");
            assert_eq!(h.core.current_workspace().unwrap().project, q);
            assert_eq!(h.core.current_workspace().unwrap().name, "c");
        }
    }

    #[test]
    fn tab_ignores_childless_and_informational_rows_and_styles_markers() {
        let mut h = workspaces();
        let empty = h
            .core
            .add_project(Loaded::ad_hoc("empty", &h.path("empty")));
        h.keys("C-x t l");
        let original = h.text();
        let blank = h.core.current().workspace_rows()[3].0 - 1;
        for line in [3, 4, blank] {
            let at = h.core.current().char_idx(line, 0).unwrap();
            h.core
                .windows
                .window
                .set_point(&mut h.core.buffers, at)
                .unwrap();
            h.keys("TAB");
            assert_eq!((h.text(), h.point()), (original.clone(), at));
        }
        select(&mut h, Row::Project(empty));
        let point = h.point();
        h.keys("TAB");
        assert_eq!((h.text(), h.point()), (original, point));
        assert_eq!(
            contents(&h).last().unwrap(),
            "empty /aiditor-test/empty · no workspaces"
        );
        for light in [false, true] {
            if light {
                h.core
                    .settings
                    .theme
                    .faces
                    .get_mut(&FaceName::SessionThought)
                    .unwrap()
                    .fg = Some(0x928374);
            }
            let fg = h.core.settings.theme.face(FaceName::SessionThought).fg;
            let frame = h.frame(1440., 900.);
            for marker in ["▾", "▸"] {
                assert!(
                    frame.windows[0]
                        .display
                        .lines
                        .iter()
                        .flat_map(|l| &l.spans)
                        .any(|s| s.text.trim() == marker && s.face.fg == fg)
                );
            }
        }
    }

    #[test]
    fn tab_redraws_the_shared_list_in_splits_and_keeps_the_selected_name() {
        let mut h = workspaces();
        let current = h.core.workspace().unwrap();
        h.keys("C-x t l C-x 2 TAB");
        assert_eq!(row(&h), Row::Workspace(current));
        assert_eq!(h.core.current().line_col(h.point()).unwrap().1, 6);
        let frame = h.frame(1440., 900.);
        assert_eq!(frame.windows.len(), 2);
        for window in &frame.windows {
            assert!(window.display.lines.iter().any(|l| {
                l.spans
                    .iter()
                    .map(|s| s.text.as_str())
                    .collect::<String>()
                    .starts_with("  ▸ * b")
            }));
            assert!(window.display.lines.iter().any(|l| {
                l.spans
                    .iter()
                    .map(|s| s.text.as_str())
                    .collect::<String>()
                    .starts_with("  ▸   c")
            }));
            assert!(window.display.mode_line.text().ends_with("/5"));
        }
        h.keys("TAB");
        assert_eq!(row(&h), Row::Workspace(current));
        for window in h.frame(1440., 900.).windows {
            assert!(window.display.lines.iter().any(|l| {
                l.spans
                    .iter()
                    .map(|s| s.text.as_str())
                    .collect::<String>()
                    .trim()
                    == "notes.txt"
            }));
        }
    }

    #[test]
    fn refresh_retains_folds_by_identity_and_defaults_new_rows() {
        let mut h = workspaces();
        let a = h.core.workspaces[0].id;
        let project = h.core.current_workspace().unwrap().project;
        h.keys("C-x t l");
        select(&mut h, Row::Workspace(a));
        h.keys("TAB");
        let current = h.core.workspace().unwrap();
        let added = h.core.new_workspace("added", project);
        h.core.switch_workspace(current).unwrap();
        h.core
            .workspaces
            .iter_mut()
            .find(|w| w.id == a)
            .unwrap()
            .name = "renamed".into();
        h.keys("C-x t l");
        assert!(contents(&h).iter().any(|s| s == "▾ renamed 1 buffer"));
        assert!(contents(&h).iter().any(|s| s == "added 0 buffers"));
        select(&mut h, Row::Workspace(added));
        h.keys("RET");
        assert_eq!(h.core.workspace().unwrap(), added);
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
            h.core
                .buffers
                .get_mut(gone)
                .unwrap()
                .file
                .as_mut()
                .unwrap()
                .deleted = true;
            let conflict = h.core.buffers.find_here("conflict.rs").unwrap();
            h.core.switch_to(dirty);
            h.core
                .buffers
                .get_mut(conflict)
                .unwrap()
                .file
                .as_mut()
                .unwrap()
                .conflict = Some(0);
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
                &contents(&h)[3..],
                [
                    "▾ p /aiditor-test",
                    "▾ * task 7 buffers · 1 unsaved",
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
            assert_eq!(display.lines[cursor_line].number, 4);
            assert!(
                display.lines[cursor_line]
                    .decorations
                    .contains(&Decoration::Background(FaceName::CompletionSelected))
            );
            let where_columns: Vec<_> = display
                .lines
                .iter()
                .filter(|l| [4, 5, 9, 10].contains(&l.number))
                .map(|l| {
                    let mut column = 0;
                    for span in &l.spans {
                        if [
                            "7 buffers · 1 unsaved",
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
            assert_eq!((display.lines[line].number, column), (3, 2));
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
            buffer.file.as_mut().unwrap().conflict = Some(0);
            h.keys("C-x t l");
            assert_eq!(
                contents(&h)[7],
                "dirty.rs src/ ● unsaved ⊘ read-only [conflict]"
            );
            let display = h.frame(1440., 900.).windows.remove(0).display;
            let row = display.lines.iter().find(|l| l.number == 7).unwrap();
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
            buffer.file.as_mut().unwrap().conflict = None;
            buffer.set_saved();
            h.keys("C-x t l");
            assert_eq!(contents(&h)[4], "▾ * task 7 buffers");
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
            .new_workspace("two", h.core.current_workspace().unwrap().project);
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
        select(&mut h, Row::Workspace(workspace));
        h.keys("TAB");
        select(&mut h, Row::Workspace(second));
        h.keys("TAB");
        let lines = contents(&h);
        assert_eq!(lines.iter().filter(|l| l.ends_with(" src/")).count(), 2);
        assert!(
            lines
                .iter()
                .any(|l| l == "▾ first\\nproject /aiditor-test/first")
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
                "▾ p /aiditor-test",
                "▸ a 1 buffer",
                "▾ * b 1 buffer",
                "notes.txt",
                "",
                "▾ q /aiditor-test",
                "▸ c 1 buffer"
            ]
        );
        assert_eq!(row(&h), Row::Workspace(h.core.workspace().unwrap()));
        assert!(h.core.current().read_only);
        assert_eq!(
            (h.core.current().path(), h.core.current().modified()),
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
        let launch = h.core.workspace().unwrap();
        let first = h.core.add_project(Loaded::ad_hoc("app", &h.path("first")));
        let second = h.core.add_project(Loaded::ad_hoc("app", &h.path("second")));
        let a = h.core.new_workspace("a\nnext", first);
        let b = h.core.new_workspace("b", second);
        h.open("tab\tname");
        h.core.kill_workspace(launch).unwrap();
        h.core.projects.retain(|p| p.project.name != "home");
        h.keys("C-x t l");
        assert_eq!(
            contents(&h),
            [
                "▾ app /aiditor-test/first",
                "a\\nnext 0 buffers",
                "",
                "▾ app /aiditor-test/second",
                "▾ * b 1 buffer",
                "tab\\tname /aiditor-test/"
            ]
        );
        assert_eq!(row(&h), Row::Workspace(b));
        h.keys("p p RET");
        assert_eq!(h.core.workspace().unwrap(), a);
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
            &rows[rows.len() - 3..],
            [
                "alpha /aiditor-test/alpha · no workspaces",
                "",
                "zeta /aiditor-test/zeta · no workspaces"
            ]
        );
        h.keys("n n n RET");
        assert_eq!(h.core.current_workspace().unwrap().project, alpha);
        assert_eq!(h.core.current_workspace().unwrap().name, "alpha");
        assert_eq!(h.core.current().name, "*scratch*<alpha>");
        h.core.switch_to_workspace("b").unwrap();
        assert_eq!(h.core.current().name, "notes.txt");
    }

    #[test]
    fn project_activation_uses_identity_and_the_most_recent_workspace() {
        let mut h = workspaces();
        let project = h.core.current_workspace().unwrap().project;
        let other = h.core.add_project(Loaded::ad_hoc("p", &h.path("other")));
        h.core.new_workspace("other", other);
        h.core.switch_to_workspace("a").unwrap();
        h.keys("body C-x 2 C-b");
        let point = h.point();
        let selected = h.core.windows.selected;
        h.core.switch_to_workspace("other").unwrap();
        h.keys("C-x t l M-< RET");
        assert_eq!(h.core.current_workspace().unwrap().project, project);
        assert_eq!(h.core.current_workspace().unwrap().name, "a");
        assert_eq!(h.core.windows.tree.leaves().len(), 2);
        assert_eq!(h.core.windows.selected, selected);
        assert_eq!((h.text(), h.point()), ("body".into(), point));
        h.keys("C-x t l M-< RET");
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
        h.keys("C-x t l M-< n");
        let line = h.frame(1400., 600.).windows[0].display.mode_line.text();
        assert_eq!(line, "*Workspaces* │ 5 projects, 4 workspaces  2/9");
        h.keys("TAB");
        let buffer_line = h.core.current().workspace_rows()[1].0 + 1;
        let at = h.core.current().char_idx(buffer_line, 0).unwrap();
        h.core
            .windows
            .window
            .set_point(&mut h.core.buffers, at)
            .unwrap();
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
                "*Workspaces* │ 6 projects, 4 workspaces  2/10"
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
        let a = h.core.workspace().unwrap();
        let selected = h.core.windows.selected;
        let point = h.point();
        h.core.switch_to_workspace("b").unwrap();
        let b = h.core.workspace().unwrap();
        h.keys("C-x t l");
        h.keys("p RET");
        assert_eq!(h.core.workspace().unwrap(), a);
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
        let workspace = h.core.workspace().unwrap();
        let list = h.core.windows.window.buffer();
        for line in [3, 4] {
            let at = h.core.current().char_idx(line, 0).unwrap();
            let window = &h.core.windows.window;
            window.set_point(&mut h.core.buffers, at).unwrap();
            h.keys("RET");
            assert_eq!(h.core.message(), Some("Select a project or workspace row"));
            assert_eq!(
                (h.core.workspace().unwrap(), h.core.windows.window.buffer()),
                (workspace, list)
            );
        }
        h.keys("p");
        assert_eq!(row(&h), Row::Workspace(workspace));
        h.keys("p");
        let Row::Workspace(target) = row(&h) else {
            panic!("workspace selected")
        };
        h.core.kill_workspace(target).unwrap();
        h.keys("RET");
        assert_eq!(h.core.message(), Some("No such workspace"));
        assert_eq!(
            (h.core.workspace().unwrap(), h.core.windows.window.buffer()),
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
            assert_eq!(row(&h), Row::Workspace(h.core.workspace().unwrap()));
        }
    }

    #[test]
    fn refresh_reuses_the_list_and_workspace_teardown_releases_it() {
        let mut h = workspaces();
        let b = h.core.workspace().unwrap();
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
        assert_eq!(&contents(&h)[3..5], ["added.txt", "notes.txt"]);
        let notes = h.core.buffers.find_here("notes.txt").unwrap();
        h.core.kill_buffer(notes);
        h.core.rename_workspace("renamed").unwrap();
        h.keys("C-x t l");
        assert_eq!(
            &contents(&h)[2..6],
            ["▾ * renamed 1 buffer", "added.txt", "", "▾ q /aiditor-test"]
        );
        h.core.kill_workspace(b).unwrap();
        assert_eq!(h.core.windows.window.buffer(), other_list);
        h.keys("C-x t l q C-x t l");
        assert_eq!(h.core.current().name, "*Workspaces*");
        assert_eq!(h.core.current().workspace_rows().len(), 4);
    }

    fn mode_line(h: &mut Harness) -> String {
        h.frame(800., 600.).windows[0].display.mode_line.text()
    }

    #[test]
    fn the_empty_lobby_list_says_no_projects() {
        let mut h = Harness::lobby(&[]);
        h.keys("g");
        assert_eq!(
            (h.core.current().name.as_str(), h.text().as_str()),
            ("*Workspaces*", "No projects")
        );
        assert_eq!(
            mode_line(&mut h).trim_end(),
            "*Workspaces* │ 0 projects, 0 workspaces"
        );
        h.keys("n p TAB");
        assert_eq!((h.core.message(), h.point()), (None, 0));
    }

    #[test]
    fn the_lobby_list_starts_on_its_first_project_and_ret_leaves_the_lobby() {
        let mut h = Harness::lobby(&[]);
        for name in ["alpha", "beta"] {
            let project = Loaded::ad_hoc(name, &h.path(name));
            h.core.add_project(project);
        }
        h.keys("g");
        assert_eq!(row(&h), Row::Project(h.core.project_id("alpha").unwrap()));
        assert!(mode_line(&mut h).ends_with("2 projects, 0 workspaces  1/2"));
        // `q` and `C-g` have nothing to go back to.
        h.keys("q C-g");
        assert_eq!(
            (h.core.workspace(), h.core.current().name.as_str()),
            (None, "*Workspaces*")
        );
        h.keys("n RET");
        assert_eq!(h.core.current().name, "*scratch*<beta>");
        let workspace = h.core.current_workspace().unwrap();
        assert_eq!(h.core.project_name(workspace.project), "beta");
        assert!(h.core.buffers.find("*Workspaces*").is_none());
    }

    #[test]
    fn the_lobby_hints_name_the_ways_out() {
        let mut h = Harness::lobby(&[]);
        assert_eq!(h.core.frame().key_hints, shown(LOBBY));
        let main = h.new_workspace("w", "p");
        h.keys("C-x t l");
        assert_eq!(
            (h.core.workspace(), h.core.frame().key_hints),
            (Some(main), shown(LIST))
        );
    }
}
