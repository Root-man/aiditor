//! The diff view (M9): a temporary full-frame layout of the current workspace, the changed-file
//! list on the left and a window for the diff on the right.

use std::collections::{BTreeMap, BTreeSet};
use std::ops::Range;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use ropey::Rope;

use crate::buffer::{Buffer, BufferId, Face, FaceRef, Kind, Prop};
use crate::command::Args;
use crate::display::Segment;
use crate::settings::FaceName;
use crate::timer::TimerId;
use crate::window::{DiffRow, Tree, Window, WindowId};
use crate::workspace::{Windows, WorkspaceId};
use crate::{
    ChangedFile, Core, Ctx, Effect, Effects, FileDiff, FileOp, GitChange, GitFile, GitSide,
    GitStatus, GitStep, Hunk, Msg, VisitIntent,
};

pub(crate) const MODE: &str = "diff-list-mode";
/// The list's share of the frame: frame 15's 460 of 1440 px.
const RATIO: f32 = 0.32;

/// A file service read: the bytes and hash, `None` for no file.
type Read = Result<Option<(Vec<u8>, u64)>, String>;

/// The pause after an edit before the shown diff is worked out again.
const REDIFF: Duration = Duration::from_millis(50);

/// The echo line's actions in the list; `REVISIONS_HINTS` picks entries by index.
pub(crate) const HINTS: &[(&[&str], &str)] = &[
    (&["diff-next-change", "diff-previous-change"], "change"),
    (&["diff-next-file", "diff-previous-file"], "file"),
    (&["diff-copy-removed"], "copy old lines"),
    (&["diff-stage", "diff-unstage"], "stage/unstage"),
    (&["diff-discard"], "discard"),
    (&["diff-toggle"], "fold"),
    (&["diff-refresh"], "refresh"),
    (&["diff-quit"], "quit"),
];

/// `HINTS` without staging and discarding, which a revisions diff refuses.
const REVISIONS_HINTS: &[(&[&str], &str)] =
    &[HINTS[0], HINTS[1], HINTS[2], HINTS[5], HINTS[6], HINTS[7]];

/// A line of the list point can be kept on across a refresh.
#[derive(Clone, Debug, PartialEq)]
enum Row {
    Root(PathBuf),
    /// A `Staged` or `Changes` heading.
    Group {
        root: PathBuf,
        staged: bool,
    },
    File {
        root: PathBuf,
        staged: bool,
        path: PathBuf,
        /// The repository's top directory and the file's path from it.
        repository: PathBuf,
        file: PathBuf,
    },
}

/// What `diff-stage`, `diff-unstage` and `diff-discard` do.
#[derive(Clone, Copy, PartialEq)]
pub(crate) enum Action {
    Stage,
    Unstage,
    Discard,
}

/// A revisions list's root's repository and revisions, `new` `None` for the working tree.
pub(crate) struct Revisions {
    repository: PathBuf,
    old: String,
    new: Option<String>,
    /// The file listing awaited.
    request: u64,
}

/// A `diff-revisions` waiting on the file service or its prompts.
pub(crate) enum Pending {
    Refs {
        request: u64,
        workspace: WorkspaceId,
    },
    /// Each git root with its repository and refs, for the prompts.
    Roots {
        workspace: WorkspaceId,
        roots: Vec<(PathBuf, PathBuf, Vec<String>)>,
    },
    Files {
        workspace: WorkspaceId,
        root: PathBuf,
        revisions: Revisions,
    },
}

/// A side as the mode line names it.
fn side_name(side: &GitSide) -> &str {
    match side {
        GitSide::Tree(tree) => tree,
        GitSide::Index => "index",
        GitSide::WorkingTree => "working tree",
    }
}

/// A file listing as a status answer of one root, every file an unstaged change.
fn revisions_answer(
    root: PathBuf,
    repository: &Path,
    result: Result<Vec<ChangedFile>, String>,
) -> Answer {
    let files = |files: Vec<ChangedFile>| {
        (files.into_iter())
            .map(|f| GitFile {
                path: f.path,
                staged: None,
                unstaged: Some(f.change),
                old_path: f.old_path,
            })
            .collect()
    };
    let status = result.map(|f| {
        Some(GitStatus {
            repository: repository.to_path_buf(),
            branch: String::new(),
            files: files(f),
        })
    });
    vec![(root, status)]
}

/// The diff the view's right window shows.
struct Shown {
    /// A `Row::File`.
    row: Row,
    /// The old and new sides.
    sides: (GitSide, GitSide),
    /// The answer awaited; 0 for none.
    request: u64,
    /// The `FileOp::Open` awaited for the file's buffer.
    visit: Option<u64>,
    /// The old side's text, once answered.
    old: Option<Rope>,
    /// A hidden buffer holding the old text in the file's grammar, for the removed rows' faces.
    old_buffer: Option<BufferId>,
    /// The new side's buffer, once there.
    buffer: Option<BufferId>,
    /// A buffer the view made for the index text or a reason, killed with the diff.
    side: Option<BufferId>,
    hunks: Vec<Hunk>,
    /// The new side's tick the hunks were last asked for; `None` before the first ask.
    tick: Option<u64>,
    timer: Option<TimerId>,
    /// Whether point went to the first change, done once when the hunks first arrive.
    jumped: bool,
}

impl Shown {
    fn staged(&self) -> bool {
        matches!(self.row, Row::File { staged: true, .. })
    }

    /// Whether the new side is not the working tree, so a read-only buffer holds it.
    fn fixed(&self) -> bool {
        self.sides.1 != GitSide::WorkingTree
    }

    /// Where change `h` starts in a new side whose last line is `last`: removed rows past the
    /// end sit on the last line.
    fn start(h: &Hunk, last: usize) -> usize {
        h.new.start.min(last)
    }

    /// The change point's `line` is in: its new lines, or the line its removed rows sit on.
    fn change_at(&self, (line, last): (usize, usize)) -> Option<usize> {
        (self.hunks.iter()).position(|h| h.new.contains(&line) || Self::start(h, last) == line)
    }
}

type Answer = Vec<(PathBuf, Result<Option<GitStatus>, String>)>;

/// A changed-file list's state.
pub(crate) struct DiffList {
    roots: Vec<PathBuf>,
    /// The latest status shown; `None` until the first arrives.
    answer: Option<Answer>,
    /// Statuses asked and not yet answered.
    pending: usize,
    /// A stage, unstage or discard is in git, or its refreshed status hasn't been taken.
    updating: bool,
    rows: Vec<(usize, Row)>,
    /// What `TAB` folded: a root (`None`) or its `Staged`/`Changes` group.
    folded: BTreeSet<(PathBuf, Option<bool>)>,
    /// The view's right window.
    window: WindowId,
    shown: Option<Box<Shown>>,
    /// The shown diff's row and point's line and column before a refresh, to return to.
    near: Option<Box<(Row, (usize, usize))>>,
    /// Set for a `diff-revisions` list, whose one root is `roots[0]`.
    revisions: Option<Revisions>,
}

/// Where a file row's path starts: its indent, letter and gap.
const PATH_COLUMN: usize = 9;

impl DiffList {
    /// Cuts each of `lines` to `cols`, a file row's path from the left with `…` so its name stays.
    pub(crate) fn fit(&self, lines: &mut [crate::display::Line], cols: usize) {
        for line in lines {
            let file =
                (self.rows.iter()).any(|(l, r)| *l == line.number && matches!(r, Row::File { .. }));
            let len: usize = line.spans.iter().map(|s| s.text.chars().count()).sum();
            // Cut one more for the `…`, keeping the name's last char at least.
            let mut cut = (len + 1)
                .saturating_sub(cols)
                .min(len.saturating_sub(PATH_COLUMN + 1));
            if file && len > cols && cut > 0 {
                let mut col = 0;
                let mut marked = false;
                for span in &mut line.spans {
                    let chars: Vec<char> = span.text.chars().collect();
                    let from = PATH_COLUMN.saturating_sub(col).min(chars.len());
                    let take = cut.min(chars.len() - from);
                    col += chars.len();
                    if from == chars.len() && take == 0 {
                        continue;
                    }
                    cut -= take;
                    let mut text: String = chars[..from].iter().collect();
                    if !marked && from + take < chars.len() {
                        text.push('…');
                        marked = true;
                    }
                    text.extend(&chars[from + take..]);
                    span.text = text;
                }
                line.spans.retain(|s| !s.text.is_empty());
            }
            crate::display::cut_spans(&mut line.spans, cols.saturating_add(1));
        }
    }
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
            self.took_answer(id);
        }
    }

    /// Shows list `id`'s new answer, asking its shown diff again.
    fn took_answer(&mut self, id: BufferId) {
        self.render_diff_list(id);
        let near = self.diff_point(id);
        // A refresh asks the shown diff again.
        self.drop_shown(id);
        if let Some(list) = self.buffers.get_mut(id).and_then(|b| b.diff_list_mut()) {
            (list.near, list.updating) = (near.map(Box::new), false);
        }
    }

    /// The echo line's actions for the current view.
    pub(crate) fn diff_hints(&self) -> &'static [(&'static [&'static str], &'static str)] {
        match self.diff_revisions_here() {
            Some(_) => REVISIONS_HINTS,
            None => HINTS,
        }
    }

    /// The current workspace's list's revisions, for a `diff-revisions` list.
    fn diff_revisions_here(&self) -> Option<&Revisions> {
        let list = self.diff_list_here()?;
        self.buffers.get(list)?.diff_list()?.revisions.as_ref()
    }

    /// Takes a file listing: opens the view `diff-revisions` asked for, or refreshes its list.
    pub(crate) fn on_git_diff_files(
        &mut self,
        request: u64,
        result: Result<Vec<ChangedFile>, String>,
    ) {
        if let Some(Pending::Files { revisions, .. }) = &self.diff_pending
            && revisions.request == request
        {
            let Some(Pending::Files {
                workspace,
                root,
                revisions,
            }) = self.diff_pending.take()
            else {
                unreachable!("matched above")
            };
            let Ok(name) = (self.current_workspace())
                .map(|w| format!("*Diff*<{}>", w.name))
                .and_then(|name| match self.workspace() == Some(workspace) {
                    true => Ok(name),
                    false => Err(String::new()),
                })
            else {
                return;
            };
            let files = match result {
                Ok(files) => files,
                Err(err) => return self.echo(err),
            };
            self.leave_diff_view();
            let answer = revisions_answer(root.clone(), &revisions.repository, Ok(files));
            let id = self.show_diff_list(None, name, Some(revisions));
            let list = self.buffers.get_mut(id).and_then(|b| b.diff_list_mut());
            let list = list.expect("just made");
            (list.roots, list.answer) = (vec![root], Some(answer));
            return self.render_diff_list(id);
        }
        let found = (self.buffers.iter_mut()).find_map(|(id, b)| {
            let list = b.diff_list_mut()?;
            list.revisions.as_ref().filter(|r| r.request == request)?;
            Some((id, list))
        });
        if let Some((id, list)) = found {
            let repository = &list.revisions.as_ref().expect("found").repository;
            list.answer = Some(revisions_answer(list.roots[0].clone(), repository, result));
            self.took_answer(id);
        }
    }

    /// List `id`'s shown row and point's line and column in its window, while it shows the diff.
    fn diff_point(&self, id: BufferId) -> Option<(Row, (usize, usize))> {
        let list = self.buffers.get(id)?.diff_list()?;
        let shown = list.shown.as_ref()?;
        let window = match list.window == self.windows.selected {
            true => &self.windows.window,
            false => self.windows.others.get(&list.window)?,
        };
        let buffer = self
            .buffers
            .get(shown.buffer.filter(|&b| b == window.buffer())?)?;
        let at = buffer.line_col(window.point(&self.buffers)).ok()?;
        Some((shown.row.clone(), at))
    }

    /// Takes a stage, unstage or discard's answer: refreshes list `list`, or echoes git's error.
    pub(crate) fn on_git_updated(
        &mut self,
        list: BufferId,
        result: Result<(), String>,
        effects: &mut Effects,
    ) {
        if let Err(err) = result {
            if let Some(list) = self.buffers.get_mut(list).and_then(|b| b.diff_list_mut()) {
                list.updating = false;
            }
            return self.echo(err);
        }
        if let Some(list) = self.buffers.get_mut(list).and_then(|b| b.diff_list_mut()) {
            list.pending += 1;
            effects.push(Effect::File(FileOp::GitStatus(list.roots.clone())));
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
        let line = buffer
            .line_col(self.list_point(id))
            .map_or(0, |(line, _)| line);
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
                    let has_files = matches!(result, Ok(Some(s)) if !s.files.is_empty());
                    let folded = has_files && list.folded.contains(&(root.clone(), None));
                    let mark = match (has_files, folded) {
                        (false, _) => "  ",
                        (true, false) => "▾ ",
                        (true, true) => "▸ ",
                    };
                    let mut heading =
                        vec![(mark.into(), face(FaceName::ModeLineDim)), (name, bold())];
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
                    heading.push(match &list.revisions {
                        Some(r) => (
                            format!("{} → {}", r.old, r.new.as_deref().unwrap_or("working tree")),
                            face(FaceName::ModeLineDim),
                        ),
                        None => (format!("⎇ {}", status.branch), face(FaceName::VcsBranch)),
                    });
                    lines.push(heading);
                    if status.files.is_empty() {
                        lines.push(vec![("    No changes".into(), face(FaceName::Annotation))]);
                    }
                    if folded {
                        continue;
                    }
                    // A revisions diff's files sit straight under their root.
                    let groups: &[(bool, &str)] = match list.revisions {
                        Some(_) => &[(false, "")],
                        None => &[(true, "Staged"), (false, "Changes")],
                    };
                    for &(staged, group) in groups {
                        let files: Vec<_> = (status.files.iter())
                            .filter_map(|f| Some((if staged { f.staged } else { f.unstaged }?, f)))
                            .collect();
                        if files.is_empty() {
                            continue;
                        }
                        let folded = list.folded.contains(&(root.clone(), Some(staged)));
                        if list.revisions.is_none() {
                            lines.push(vec![
                                (
                                    if folded { "  ▸ " } else { "  ▾ " }.into(),
                                    face(FaceName::ModeLineDim),
                                ),
                                (group.into(), face(FaceName::DiffListGroup)),
                                // Counts right-aligned to 3 digits, past the longer name, line up.
                                (" ".repeat(9 - group.len()), None),
                                (
                                    format!("{:>3}", files.len()),
                                    face(FaceName::SessionListCount),
                                ),
                            ]);
                            rows.push((
                                lines.len() - 1,
                                Row::Group {
                                    root: root.clone(),
                                    staged,
                                },
                            ));
                        }
                        if folded {
                            continue;
                        }
                        for (change, file) in files {
                            let full = status.repository.join(&file.path);
                            let path = (full.strip_prefix(root))
                                .map_or_else(|_| file.path.clone(), Path::to_path_buf);
                            let (letter, letter_face) = letter(change);
                            let shown = path.display().to_string();
                            let (dir, name) = shown.split_at(shown.rfind('/').map_or(0, |i| i + 1));
                            lines.push(vec![
                                ("      ".into(), None),
                                (letter.into(), face(letter_face)),
                                ("  ".into(), None),
                                (dir.into(), face(FaceName::ModeLineDim)),
                                (name.into(), None),
                            ]);
                            let row = Row::File {
                                root: root.clone(),
                                staged,
                                path,
                                repository: status.repository.clone(),
                                file: file.path.clone(),
                            };
                            rows.push((lines.len() - 1, row));
                        }
                    }
                }
            }
        }
        // A row that left: the nearest file listed after it and still listed, else before it.
        let listed = |r: &Row| rows.iter().find(|(_, n)| n == r);
        let gone = at
            .as_ref()
            .and_then(|at| list.rows.iter().position(|(_, r)| r == at));
        let near = gone.and_then(|i| {
            let (before, after) = list.rows.split_at(i);
            (after.iter().chain(before.iter().rev()))
                .filter(|(_, r)| matches!(r, Row::File { .. }))
                .find_map(|(_, r)| listed(r))
        });
        let target = (at.as_ref().and_then(listed))
            .or(near)
            .or_else(|| rows.iter().find(|(_, r)| matches!(r, Row::File { .. })))
            .map(|(l, _)| *l);
        let buffer = self.buffers.get_mut(id).expect("checked above");
        buffer.diff_list_mut().expect("checked above").rows = rows;
        buffer.set_lines(lines);
        let point = target.map_or(0, |line| buffer.char_idx(line, 0).expect("a row's line"));
        self.place_list_point(id, point, false);
    }
}

/// The diff rows of `hunks`' removed lines of `old` over a new side of `lines` lines: each above
/// the line that replaced it, after the last when the removal ended the file, in `old`'s syntax
/// faces from `palette` on `diff-removed`'s background.
fn removed_rows(
    old: &Buffer,
    hunks: &[Hunk],
    lines: usize,
    palette: &BTreeMap<String, Face>,
) -> Vec<(usize, DiffRow)> {
    let (text, last) = (old.text(), lines.saturating_sub(1));
    let to_char = |i: usize| text.line_to_char(i.min(text.len_lines()));
    // One range per hunk, so a large deletion is painted in one pass.
    let ranges: Vec<Range<usize>> = (hunks.iter())
        .map(|h| to_char(h.old.start)..to_char(h.old.end))
        .filter(|r| !r.is_empty())
        .collect();
    let faces = old.highlights(&ranges, palette);
    let run = |r: Range<usize>, refs: Vec<FaceRef>| (text.slice(r).to_string(), refs);
    let mut next = 0;
    (hunks.iter())
        .flat_map(|h| h.old.clone().map(move |i| (h.new.start, i)))
        .filter(|(_, i)| *i < text.len_lines())
        .map(|(at, i)| {
            let line = text.line(i).to_string();
            let start = text.line_to_char(i);
            let end = start + line.trim_end_matches(['\n', '\r']).chars().count();
            while faces.get(next).is_some_and(|(r, _)| r.end <= start) {
                next += 1;
            }
            let (mut runs, mut from) = (Vec::new(), start);
            for (r, is) in faces[next..].iter().take_while(|(r, _)| r.start < end) {
                let r = r.start.max(start)..r.end.min(end);
                runs.push(run(from..r.start, Vec::new()));
                from = r.end;
                runs.push(run(r, vec![FaceRef::Is(*is)]));
            }
            runs.push(run(from..end, Vec::new()));
            runs.retain(|(t, _)| !t.is_empty());
            let row = DiffRow {
                after: at > last,
                runs,
                label: None,
                sign: Some(('-', FaceName::DiffRemoved)),
                height: 1,
            };
            (at.min(last), row)
        })
        .collect()
}

/// What `frame` adds for the diff window: its id, the added lines and the mode line's segments.
pub(crate) struct Decor {
    pub(crate) window: WindowId,
    pub(crate) added: Vec<Range<usize>>,
    pub(crate) left: Vec<Segment>,
    pub(crate) right: Vec<Segment>,
}

impl Core {
    /// The list whose shown diff awaits `request`, and that diff.
    fn awaiting(&mut self, request: u64) -> Option<(BufferId, WindowId, &mut Shown)> {
        (self.buffers.iter_mut()).find_map(|(id, b)| {
            let list = b.diff_list_mut()?;
            let window = list.window;
            let shown = list.shown.as_deref_mut().filter(|s| s.request == request)?;
            Some((id, window, shown))
        })
    }

    fn shown_mut(&mut self, list: BufferId) -> Option<&mut Shown> {
        self.buffers
            .get_mut(list)?
            .diff_list_mut()?
            .shown
            .as_deref_mut()
    }

    /// Forgets list `list`'s shown diff, killing its side buffer and timer.
    fn drop_shown(&mut self, list: BufferId) {
        let shown = (self.buffers.get_mut(list)).and_then(|b| b.diff_list_mut()?.shown.take());
        let Some(shown) = shown else {
            return;
        };
        if let Some(timer) = shown.timer {
            self.timers.cancel(timer);
        }
        for side in [shown.side, shown.old_buffer].into_iter().flatten() {
            if self.buffers.get(side).is_some() {
                self.kill_buffer(side);
            }
        }
    }

    /// After each message: shows the file at point in the current workspace's list, and asks
    /// for the shown diff again a moment after its buffer changes.
    pub(crate) fn sync_diff(&mut self, now: Instant, effects: &mut Effects) {
        let Some(id) = self
            .diff_list_here()
            .filter(|_| self.windows.saved.is_some())
        else {
            return;
        };
        let list = self
            .buffers
            .get(id)
            .and_then(|b| b.diff_list())
            .expect("found");
        let shown_window = (self.windows.iter()).find(|(_, w)| w.buffer() == id);
        let row = shown_window.and_then(|(_, w)| {
            let buffer = self.buffers.get(id)?;
            let line = buffer.line_col(w.point(&self.buffers)).ok()?.0;
            (list.rows.iter()).find(|(l, r)| *l == line && matches!(r, Row::File { .. }))
        });
        if let Some((_, row)) = row
            && list.shown.as_ref().is_none_or(|s| s.row != *row)
        {
            let row = row.clone();
            self.show_diff(id, row, effects);
        }
        let Some(shown) = self
            .buffers
            .get(id)
            .and_then(|b| b.diff_list()?.shown.as_ref())
        else {
            return;
        };
        let tick = shown
            .buffer
            .and_then(|b| self.buffers.get(b))
            .map(|b| b.tick());
        let Some(tick) =
            tick.filter(|&t| shown.old.is_some() && !shown.fixed() && shown.tick != Some(t))
        else {
            return;
        };
        if shown.tick.is_none() {
            return self.ask_lines(id, effects);
        }
        let timer = self.timers.start(now, REDIFF, None, Msg::Rediff(id));
        let shown = self.shown_mut(id).expect("checked above");
        let old = shown.timer.replace(timer);
        // An answer for the text before this edit is stale.
        (shown.tick, shown.request) = (Some(tick), 0);
        if let Some(old) = old {
            self.timers.cancel(old);
        }
    }

    /// Takes the old and new sides of a shown diff.
    pub(crate) fn on_git_diffed(
        &mut self,
        request: u64,
        result: Result<FileDiff, String>,
        effects: &mut Effects,
    ) {
        let Some((list, window, shown)) = self.awaiting(request) else {
            return;
        };
        shown.request = 0;
        let (fixed, new) = (shown.fixed(), side_name(&shown.sides.1).to_string());
        let Row::File {
            path,
            repository,
            file,
            ..
        } = shown.row.clone()
        else {
            return;
        };
        let diff = match result {
            Ok(diff) => diff,
            Err(err) => {
                let id = self.side_buffer(list, "*Diff file*", &err);
                return self.attach(list, window, id);
            }
        };
        let old = self.buffers.create(" *diff old*");
        let buffer = self.buffers.get_mut(old).expect("just created");
        buffer.set_text(&diff.old.to_string());
        buffer.set_grammar(&file);
        let workspace = self.buffers.get(list).expect("a live list").workspace;
        self.buffers.get_mut(old).expect("just created").workspace = workspace;
        let shown = self.shown_mut(list).expect("awaited");
        (shown.old, shown.old_buffer) = (Some(diff.old), Some(old));
        if fixed {
            shown.hunks = diff.hunks;
            let name = format!("{} ({new})", path.display());
            let id = self.side_buffer(list, &name, &diff.new.to_string());
            self.buffers
                .get_mut(id)
                .expect("just made")
                .set_grammar(&file);
            self.attach(list, window, id);
            return self.first_change(list);
        }
        let full = repository.join(file);
        let workspace = self.buffers.get(list).expect("a live list").workspace;
        match self.buffers.visiting_in(&full, workspace) {
            Some(id) => self.attach(list, window, id),
            None => {
                self.visits += 1;
                let intent = VisitIntent {
                    request: self.visits,
                    workspace,
                    window,
                };
                self.shown_mut(list).expect("awaited").visit = Some(intent.request);
                let path = full;
                effects.push(Effect::File(FileOp::Open { intent, path }));
            }
        }
    }

    /// Starts showing `row`'s diff for list `list`.
    fn show_diff(&mut self, list: BufferId, row: Row, effects: &mut Effects) {
        self.drop_shown(list);
        let Row::File {
            staged,
            repository,
            file,
            ..
        } = &row
        else {
            return;
        };
        let revisions = (self.buffers.get(list)).and_then(|b| b.diff_list()?.revisions.as_ref());
        let (old, new) = match (revisions, staged) {
            (Some(r), _) => (
                GitSide::Tree(r.old.clone()),
                (r.new.clone()).map_or(GitSide::WorkingTree, GitSide::Tree),
            ),
            (None, true) => (GitSide::Tree("HEAD".into()), GitSide::Index),
            (None, false) => (GitSide::Index, GitSide::WorkingTree),
        };
        self.visits += 1;
        let request = self.visits;
        effects.push(Effect::File(FileOp::GitDiff {
            request,
            repository: repository.clone(),
            path: file.clone(),
            old: old.clone(),
            new: new.clone(),
            objects: None,
        }));
        let diff = self.buffers.get_mut(list).and_then(|b| b.diff_list_mut());
        diff.expect("a live list").shown = Some(Box::new(Shown {
            row,
            sides: (old, new),
            request,
            visit: None,
            old: None,
            old_buffer: None,
            buffer: None,
            side: None,
            hunks: Vec::new(),
            tick: None,
            timer: None,
            jumped: false,
        }));
    }

    /// A read-only buffer of `text` named `name`, in list `list`'s workspace, as its side buffer.
    fn side_buffer(&mut self, list: BufferId, name: &str, text: &str) -> BufferId {
        let workspace = self.buffers.get(list).expect("a live list").workspace;
        let id = self.buffers.create(name);
        let buffer = self.buffers.get_mut(id).expect("just created");
        buffer.set_text(text);
        (buffer.read_only, buffer.workspace) = (true, workspace);
        self.shown_mut(list).expect("showing").side = Some(id);
        id
    }

    /// Shows buffer `id` as the new side of list `list`'s diff in its window `window`.
    fn attach(&mut self, list: BufferId, window: WindowId, id: BufferId) {
        self.shown_mut(list).expect("showing").buffer = Some(id);
        if let Some(window) = view_window(&mut self.windows, &mut self.workspaces, window) {
            window.show(&mut self.buffers, id);
        }
        self.redraw(list);
    }

    /// Redraws every list's removed rows, after a parse or a theme change recolours them.
    pub(crate) fn redraw_diffs(&mut self) {
        let lists: Vec<BufferId> = (self.buffers.iter_mut())
            .filter_map(|(id, b)| b.diff_list().map(|_| id))
            .collect();
        for list in lists {
            self.redraw(list);
        }
    }

    /// Puts list `list`'s removed rows in its window, if it shows the diff's buffer.
    fn redraw(&mut self, list: BufferId) {
        let Some(diff) = self.buffers.get(list).and_then(|b| b.diff_list()) else {
            return;
        };
        let Some(shown) = &diff.shown else {
            return;
        };
        let (Some(old), Some(id)) = (shown.old_buffer, shown.buffer) else {
            return;
        };
        let (Some(old), Some(lines)) = (
            self.buffers.get(old),
            self.buffers.get(id).map(|b| b.len_lines()),
        ) else {
            return;
        };
        let rows = removed_rows(old, &shown.hunks, lines, &self.settings.theme.syntax);
        let window = diff.window;
        let window = view_window(&mut self.windows, &mut self.workspaces, window);
        if let Some(window) = window.filter(|w| w.buffer() == id) {
            window.set_diff_rows(&mut self.buffers, rows);
        }
    }

    /// Takes the hunks of a re-diff.
    pub(crate) fn on_lines_diffed(&mut self, request: u64, hunks: Vec<Hunk>) {
        let Some((list, _, shown)) = self.awaiting(request) else {
            return;
        };
        (shown.hunks, shown.request) = (hunks, 0);
        self.redraw(list);
        self.first_change(list);
    }

    /// Puts point in list `list`'s diff window on the first change, once, when its hunks first
    /// arrive, or back where it was when a refresh asked the same row again; later re-diffs
    /// leave point where editing put it.
    fn first_change(&mut self, list: BufferId) {
        let Some(diff) = self.buffers.get_mut(list).and_then(|b| b.diff_list_mut()) else {
            return;
        };
        let (window, near) = (diff.window, diff.near.take());
        let Some(shown) = diff.shown.as_mut().filter(|s| !s.jumped) else {
            return;
        };
        shown.jumped = true;
        let near = near.filter(|n| n.0 == shown.row).map(|n| n.1);
        let (Some(id), hunk) = (shown.buffer, shown.hunks.first().cloned()) else {
            return;
        };
        let Some(buffer) = self.buffers.get(id) else {
            return;
        };
        let last = buffer.len_lines() - 1;
        let (line, col) = match (near, hunk) {
            (Some((line, col)), _) => (line.min(last), col),
            (None, Some(hunk)) => (Shown::start(&hunk, last), 0),
            (None, None) => return,
        };
        let at = buffer.char_idx(line, col).expect("in the buffer");
        let window = view_window(&mut self.windows, &mut self.workspaces, window);
        if let Some(window) = window.filter(|w| w.buffer() == id) {
            window.jump(&mut self.buffers, at).expect("in the buffer");
        }
    }

    /// Asks for the hunks of list `list`'s diff against its buffer's text now.
    fn ask_lines(&mut self, list: BufferId, effects: &mut Effects) {
        self.visits += 1;
        let request = self.visits;
        let Some(shown) = self.shown_mut(list) else {
            return;
        };
        let (Some(old), Some(id)) = (shown.old.clone(), shown.buffer) else {
            return;
        };
        let Some(buffer) = self.buffers.get(id) else {
            return;
        };
        let (new, tick) = (buffer.text().clone(), buffer.tick());
        let shown = self.shown_mut(list).expect("checked above");
        (shown.request, shown.tick) = (request, Some(tick));
        effects.push(Effect::File(FileOp::DiffLines { request, old, new }));
    }

    /// Takes an `Opened` the view asked for, else hands `result` back.
    pub(crate) fn diff_read(&mut self, request: u64, path: &Path, result: Read) -> Option<Read> {
        let found = (self.buffers.iter_mut()).find_map(|(id, b)| {
            let list = b.diff_list_mut()?;
            let shown = list.shown.as_mut().filter(|s| s.visit == Some(request))?;
            shown.visit = None;
            Some((id, list.window, b.workspace))
        });
        let Some((list, window, workspace)) = found else {
            return Some(result);
        };
        // A buffer made meanwhile, by `C-x C-f` say, is the one to show.
        let made = match self.buffers.visiting_in(path, workspace) {
            Some(id) => Ok(id),
            None => result.and_then(|file| self.file_buffer(path, file, true)),
        };
        match made {
            Ok(id) => {
                self.buffers.get_mut(id).expect("live").workspace = workspace;
                self.attach(list, window, id);
            }
            Err(err) => {
                let id = self.side_buffer(list, "*Diff file*", &err);
                self.attach(list, window, id);
            }
        }
        None
    }

    /// The current view's list, its right window and the shown diff, while the window shows
    /// the diff's buffer.
    fn diff_shown(&self) -> Option<(BufferId, &Window, &Shown)> {
        let list = self
            .diff_list_here()
            .filter(|_| self.windows.saved.is_some())?;
        let diff = self.buffers.get(list)?.diff_list()?;
        let shown = diff.shown.as_ref().filter(|s| s.old.is_some())?;
        let window = match diff.window == self.windows.selected {
            true => &self.windows.window,
            false => self.windows.others.get(&diff.window)?,
        };
        (shown.buffer == Some(window.buffer())).then_some((list, window, shown))
    }

    /// Whether the diff window is selected, showing the diff.
    pub(crate) fn diff_selected(&self) -> bool {
        let list = self
            .diff_shown()
            .and_then(|(l, ..)| self.buffers.get(l)?.diff_list());
        list.is_some_and(|l| l.window == self.windows.selected)
    }

    /// Point's line in the diff window and the window buffer's last line.
    fn diff_line(&self, window: &Window) -> (usize, usize) {
        let buffer = self
            .buffers
            .get(window.buffer())
            .expect("window buffer is live");
        let line = buffer
            .line_col(window.point(&self.buffers))
            .map_or(0, |(l, _)| l);
        (line, buffer.len_lines() - 1)
    }

    /// The added lines and mode line segments `frame` draws in the diff window.
    pub(crate) fn diff_decor(&self) -> Option<Decor> {
        let (list, window, shown) = self.diff_shown()?;
        let line = self.diff_line(window);
        let count = |r: fn(&Hunk) -> &Range<usize>| shown.hunks.iter().map(|h| r(h).len()).sum();
        let (added, removed): (usize, usize) = (count(|h| &h.new), count(|h| &h.old));
        let sides = format!(
            "{} → {}",
            side_name(&shown.sides.0),
            side_name(&shown.sides.1)
        );
        let n = shown.hunks.len();
        let change = match shown.change_at(line) {
            Some(i) => format!("change {}/{n}", i + 1),
            None if n == 1 => "1 change".into(),
            None => format!("{n} changes"),
        };
        let diff = self.buffers.get(list)?.diff_list()?;
        let mut right = vec![vec![(change, FaceName::ModeLine)]];
        // A file's mode line names its branch; the index text's buffer has no file.
        let fileless = self.buffers.get(window.buffer())?.file.is_none();
        let root = match &shown.row {
            Row::File { root, .. } => Some(root),
            Row::Root(_) | Row::Group { .. } => None,
        };
        let status = (diff.answer.iter().flatten()).find(|(r, _)| Some(r) == root);
        if let Some((_, Ok(Some(status)))) =
            status.filter(|_| fileless && shown.sides.1 == GitSide::Index)
        {
            right.insert(
                0,
                vec![(format!("⎇ {}", status.branch), FaceName::VcsBranch)],
            );
        }
        Some(Decor {
            window: diff.window,
            added: shown.hunks.iter().map(|h| h.new.clone()).collect(),
            left: vec![
                vec![
                    // As a tool call's counts: the diff faces' backgrounds aren't the bar's.
                    (format!("+{added}"), FaceName::StatusRunning),
                    (" ".into(), FaceName::ModeLine),
                    (format!("−{removed}"), FaceName::StatusFailed),
                ],
                vec![(sides, FaceName::ModeLineDim)],
            ],
            right,
        })
    }
}

/// Window `id` of the current layout or a saved workspace's.
fn view_window<'a>(
    windows: &'a mut Windows,
    workspaces: &'a mut [crate::workspace::Workspace],
    id: WindowId,
) -> Option<&'a mut Window> {
    let saved = workspaces.iter_mut().filter_map(|w| w.windows.as_mut());
    (std::iter::once(windows).chain(saved)).find_map(|w| w.get_mut(id))
}

impl Ctx<'_> {
    /// `diff-status`: saves the workspace's layout and shows its changed-file list beside the
    /// current buffer; while the view is open, selects the list.
    pub(crate) fn diff_status(&mut self) -> Result<(), String> {
        self.outside_minibuffer("Can't show a diff in the minibuffer")?;
        let core = &mut *self.core;
        let name = format!("*Diff*<{}>", core.current_workspace()?.name);
        if core.diff_revisions_here().is_some() {
            core.leave_diff_view();
        }
        let found = core.diff_list_here();
        let list = core.show_diff_list(found, name, None);
        match found {
            Some(_) => Ok(()),
            None => {
                core.render_diff_list(list);
                self.diff_refresh()
            }
        }
    }

    /// `diff-revisions`: asks for a git root when there are several, then the old and new
    /// revisions, and opens the view once git lists the files between them.
    pub(crate) fn diff_revisions(&mut self, args: Args) -> Result<(), String> {
        self.outside_minibuffer("Can't show a diff in the minibuffer")?;
        let core = &mut *self.core;
        let workspace = core.current_workspace()?.id;
        core.visits += 1;
        let request = core.visits;
        if args.strings.is_empty() {
            core.diff_pending = Some(Pending::Refs { request, workspace });
            let roots = core.roots();
            self.effects
                .push(Effect::File(FileOp::GitRefs { request, roots }));
            return Ok(());
        }
        let Some(Pending::Roots {
            workspace: asked,
            roots,
        }) = &core.diff_pending
        else {
            return Err("No revisions asked".into());
        };
        let (root, rest) = args.strings.split_first().expect("not empty");
        let found = (roots.iter()).find(|(r, ..)| r.display().to_string() == *root);
        let (root, repository, refs) = found
            .filter(|_| *asked == workspace)
            .cloned()
            .ok_or_else(|| format!("{root}: not a git root"))?;
        let choices: Vec<&str> = refs.iter().map(String::as_str).collect();
        let command = core.commands["diff-revisions"];
        match rest {
            [] => self.read_string(command, args, "Old revision: ".into(), Some(&choices)),
            [old] if old.trim().is_empty() => Err("No revision".into()),
            [_] => {
                let prompt = "New revision (empty for working tree): ".into();
                self.read_string(command, args, prompt, Some(&choices))
            }
            [old, new, ..] => {
                let (old, new) = (old.trim().to_string(), new.trim());
                let new = (!new.is_empty()).then(|| new.to_string());
                self.effects.push(Effect::File(FileOp::GitDiffFiles {
                    request,
                    root: root.clone(),
                    old: old.clone(),
                    new: new.clone(),
                    objects: None,
                }));
                let revisions = Revisions {
                    repository,
                    old,
                    new,
                    request,
                };
                core.diff_pending = Some(Pending::Files {
                    workspace,
                    root,
                    revisions,
                });
                Ok(())
            }
        }
    }

    /// Takes the refs `diff-revisions` asked for: asks for a root among the git roots, or
    /// goes on with the only one.
    pub(crate) fn on_git_refs(&mut self, request: u64, answer: Vec<(PathBuf, crate::GitRefs)>) {
        let core = &mut *self.core;
        let Some(&Pending::Refs {
            request: asked,
            workspace,
        }) = core.diff_pending.as_ref()
        else {
            return;
        };
        if asked != request || core.workspace() != Some(workspace) {
            return;
        }
        let mut error = None;
        let roots: Vec<_> = (answer.into_iter())
            .filter_map(|(root, refs)| match refs {
                Ok(Some((repository, refs))) => Some((root, repository, refs)),
                Ok(None) => None,
                Err(err) => {
                    error.get_or_insert(err);
                    None
                }
            })
            .collect();
        let names: Vec<String> = roots
            .iter()
            .map(|(r, ..)| r.display().to_string())
            .collect();
        core.diff_pending = Some(Pending::Roots { workspace, roots });
        let result = match &names[..] {
            [] => Err(error.unwrap_or_else(|| "No git root".into())),
            [root] => {
                let strings = vec![root.clone()];
                self.diff_revisions(Args {
                    strings,
                    ..Args::default()
                })
            }
            _ => {
                let choices: Vec<&str> = names.iter().map(String::as_str).collect();
                let command = self.core.commands["diff-revisions"];
                self.read_string(command, Args::default(), "Root: ".into(), Some(&choices))
            }
        };
        if let Err(err) = result {
            self.core.echo(err);
        }
    }
}

impl Core {
    /// Shows list `found`, or a new list named `name`, as the view's left window, saving the
    /// layout when the view isn't open.
    fn show_diff_list(
        &mut self,
        found: Option<BufferId>,
        name: String,
        revisions: Option<Revisions>,
    ) -> BufferId {
        let core = self;
        let list = found.unwrap_or_else(|| {
            let id = core.buffers.create(&name);
            let buffer = core.buffers.get_mut(id).expect("just created");
            (buffer.major_mode, buffer.read_only) = (MODE, true);
            buffer.kind = Kind::DiffList(DiffList {
                roots: Vec::new(),
                answer: None,
                pending: 0,
                updating: false,
                rows: Vec::new(),
                folded: BTreeSet::new(),
                window: WindowId(core.next_window + 1),
                shown: None,
                near: None,
                revisions,
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
        list
    }
}

impl Ctx<'_> {
    /// `diff-refresh`: asks for the status of the list's roots again.
    pub(crate) fn diff_refresh(&mut self) -> Result<(), String> {
        let roots = self.core.roots();
        let id = self.core.diff_list_here().ok_or("No diff view")?;
        self.core.visits += 1;
        let request = self.core.visits;
        let buffer = self.core.buffers.get_mut(id).expect("found live");
        let list = buffer.diff_list_mut().expect("found by kind");
        // Asking with the names again resolves them again.
        if let Some(revisions) = &mut list.revisions {
            revisions.request = request;
            self.effects.push(Effect::File(FileOp::GitDiffFiles {
                request,
                root: list.roots[0].clone(),
                old: revisions.old.clone(),
                new: revisions.new.clone(),
                objects: None,
            }));
            return Ok(());
        }
        list.pending = if list.roots == roots {
            list.pending + 1
        } else {
            1
        };
        list.roots = roots.clone();
        self.effects.push(Effect::File(FileOp::GitStatus(roots)));
        Ok(())
    }

    /// The pause after an edit passed: asks for the shown diff against the text now.
    pub(crate) fn rediff(&mut self, list: BufferId) {
        if let Some(shown) = self.core.shown_mut(list) {
            shown.timer = None;
            self.core.ask_lines(list, &mut self.effects);
        }
    }

    /// `diff-next-change` and `diff-previous-change`: moves the diff window's point to the
    /// start of the next or previous change.
    pub(crate) fn diff_change(&mut self, forward: bool) -> Result<(), String> {
        let core = &mut *self.core;
        let (list, window, shown) = core.diff_shown().ok_or("No diff shown")?;
        let (line, last) = core.diff_line(window);
        let starts = shown.hunks.iter().map(|h| Shown::start(h, last));
        let target = match forward {
            true => starts.clone().find(|&s| s > line),
            false => starts.rev().find(|&s| s < line),
        };
        let target = target.ok_or(match forward {
            true => "No next change",
            false => "No previous change",
        })?;
        let id = core
            .buffers
            .get(list)
            .and_then(|b| b.diff_list())
            .expect("shown")
            .window;
        let window = core.windows.get_mut(id).expect("shown");
        let buffer = core
            .buffers
            .get(window.buffer())
            .expect("window buffer is live");
        let at = buffer.char_idx(target, 0).expect("in the buffer");
        window.jump(&mut core.buffers, at).expect("in the buffer");
        Ok(())
    }

    /// `diff-next-file` and `diff-previous-file`: moves the list's point to the next or previous
    /// file, whose diff then shows.
    pub(crate) fn diff_file(&mut self, forward: bool) -> Result<(), String> {
        let core = &mut *self.core;
        let list = (core.diff_list_here())
            .filter(|_| core.windows.saved.is_some())
            .ok_or("No diff view")?;
        let buffer = core.buffers.get(list).expect("found");
        let files = (buffer.diff_list().expect("a list").rows.iter())
            .filter(|(_, r)| matches!(r, Row::File { .. }))
            .map(|(l, _)| *l);
        let window = (std::iter::once(&mut core.windows.window))
            .chain(core.windows.others.values_mut())
            .find(|w| w.buffer() == list)
            .ok_or("No diff view")?;
        let line = buffer
            .line_col(window.point(&core.buffers))
            .map_or(0, |(l, _)| l);
        let target = match forward {
            true => files.clone().find(|&l| l > line),
            false => files.rev().find(|&l| l < line),
        };
        let target = target.ok_or(match forward {
            true => "No next file",
            false => "No previous file",
        })?;
        let at = buffer.char_idx(target, 0).expect("a row's line");
        window
            .set_point(&mut core.buffers, at)
            .expect("in the list");
        Ok(())
    }

    /// `diff-toggle`: folds or unfolds the root or group heading at or above point, point going to
    /// that heading.
    pub(crate) fn diff_toggle(&mut self) -> Result<(), String> {
        let core = &mut *self.core;
        let id = core.windows.window.buffer();
        let window = &mut core.windows.window;
        let buffer = core.buffers.get(id).expect("window buffer is live");
        let list = buffer.diff_list().ok_or("Not in a diff list")?;
        let line = (buffer.line_col(window.point(&core.buffers))).map_or(0, |(l, _)| l);
        let (heading, fold) = (list.rows.iter().rev())
            .find_map(|(l, r)| match r {
                Row::Root(root) if *l <= line => Some((*l, (root.clone(), None))),
                Row::Group { root, staged } if *l <= line => {
                    Some((*l, (root.clone(), Some(*staged))))
                }
                _ => None,
            })
            .ok_or("No root here")?;
        // A group always has files; a root may not.
        let files = (list.rows.iter()).any(|(_, r)| match r {
            Row::Group { root, .. } | Row::File { root, .. } => *root == fold.0,
            Row::Root(_) => false,
        });
        if !files && !list.folded.contains(&fold) {
            return Err("Nothing to fold".into());
        }
        let at = buffer.char_idx(heading, 0).expect("a row's line");
        let list = core
            .buffers
            .get_mut(id)
            .and_then(|b| b.diff_list_mut())
            .expect("checked");
        if !list.folded.remove(&fold) {
            list.folded.insert(fold);
        }
        window
            .set_point(&mut core.buffers, at)
            .expect("in the list");
        core.render_diff_list(id);
        Ok(())
    }

    /// `diff-stage`, `diff-unstage` and `diff-discard`: acts on the file at point in its group,
    /// or every file under the heading at point; discarding a `Changes` file asks first.
    pub(crate) fn diff_update(&mut self, action: Action, args: Args) -> Result<(), String> {
        if self.core.diff_revisions_here().is_some() {
            return Err("This diff can't be changed".into());
        }
        let list = self.core.diff_list_here();
        if (list.and_then(|id| self.core.buffers.get(id)?.diff_list())).is_some_and(|l| l.updating)
        {
            return Err("Git is still updating".into());
        }
        if self.core.diff_selected() {
            return self.diff_update_change(action);
        }
        let core = &*self.core;
        let nothing = match action {
            Action::Stage => "Nothing to stage",
            Action::Unstage => "Nothing to unstage",
            Action::Discard => "Nothing to discard",
        };
        let id = (core.diff_list_here())
            .filter(|_| core.windows.saved.is_some())
            .ok_or("No diff view")?;
        let buffer = core.buffers.get(id).expect("found");
        let list = buffer.diff_list().expect("a list");
        let (_, window) = (core.windows.iter())
            .find(|(_, w)| w.buffer() == id)
            .ok_or("No diff view")?;
        let line = (buffer.line_col(window.point(&core.buffers))).map_or(0, |(l, _)| l);
        let row = (list.rows.iter()).find(|(l, _)| *l == line).map(|(_, r)| r);
        let (root, group, file) = match row.ok_or(nothing)? {
            Row::Root(root) => (root, None, None),
            Row::Group { root, staged } => (root, Some(*staged), None),
            Row::File {
                root, staged, file, ..
            } => (root, Some(*staged), Some(file)),
        };
        let status = (list.answer.iter().flatten())
            .find(|(r, _)| r == root)
            .and_then(|(_, s)| s.as_ref().ok()?.as_ref())
            .ok_or(nothing)?;
        let pick = |staged: bool| -> Vec<&GitFile> {
            (status.files.iter())
                .filter(|f| group.is_none_or(|g| g == staged) && file.is_none_or(|p| *p == f.path))
                .filter(|f| if staged { f.staged } else { f.unstaged }.is_some())
                .collect()
        };
        let (staged, changes) = match action {
            Action::Stage => (Vec::new(), pick(false)),
            Action::Unstage => (pick(true), Vec::new()),
            Action::Discard => (pick(true), pick(false)),
        };
        if staged.is_empty() && changes.is_empty() {
            return Err(nothing.into());
        }
        let repository = &status.repository;
        let shown = |f: &GitFile| {
            let full = repository.join(&f.path);
            let path = full.strip_prefix(root).map_or(f.path.as_path(), |p| p);
            path.display().to_string()
        };
        // Git sees the disk, so a buffer's unsaved edits would be left out or lost.
        if action != Action::Unstage
            && let Some(f) = changes.iter().find(|f| {
                let full = repository.join(&f.path);
                (core.buffers.recent()).any(|(_, b)| b.path() == Some(&full) && b.modified())
            })
        {
            return Err(match action {
                Action::Stage => format!("Save {} first", shown(f)),
                _ => format!("{} has unsaved edits", shown(f)),
            });
        }
        let (deleted, restored): (Vec<&GitFile>, Vec<&GitFile>) =
            (changes.iter()).partition(|f| f.unstaged == Some(GitChange::Untracked));
        if action == Action::Discard && !changes.is_empty() {
            match args.strings.first() {
                Some(answer) if answer.eq_ignore_ascii_case("yes") => {}
                Some(_) => return Ok(()),
                None => {
                    let what = |files: &[&GitFile], verb: &str| match files {
                        [] => None,
                        [f] => Some(format!("{verb} {}", shown(f))),
                        _ => Some(format!("{verb} {} files", files.len())),
                    };
                    let parts: Vec<_> = [
                        what(&restored, "discard changes to"),
                        what(&deleted, "delete untracked"),
                    ]
                    .into_iter()
                    .flatten()
                    .collect();
                    let question = parts.join(" and ");
                    let prompt = format!("{}{}? ", question[..1].to_uppercase(), &question[1..]);
                    let command = self.core.commands["diff-discard"];
                    return self.read_string(command, args, prompt, Some(&["No", "Yes"]));
                }
            }
        }
        let paths = |files: &[&GitFile], old: bool| -> Vec<PathBuf> {
            (files.iter())
                .flat_map(|f| std::iter::once(&f.path).chain(f.old_path.as_ref().filter(|_| old)))
                .cloned()
                .collect()
        };
        let steps = match action {
            Action::Stage => vec![(GitStep::Stage, paths(&changes, false))],
            Action::Unstage => vec![(GitStep::Unstage, paths(&staged, true))],
            // Restoring first leaves a staged change in the working tree once unstaged.
            Action::Discard => vec![
                (GitStep::Restore, paths(&restored, false)),
                (GitStep::Delete, paths(&deleted, false)),
                (GitStep::Unstage, paths(&staged, true)),
            ],
        };
        let repository = repository.clone();
        self.core
            .buffers
            .get_mut(id)
            .and_then(|b| b.diff_list_mut())
            .expect("a list")
            .updating = true;
        self.effects.push(Effect::File(FileOp::GitUpdate {
            list: id,
            repository,
            steps,
        }));
        Ok(())
    }

    /// `diff-stage`, `diff-unstage` and `diff-discard` in the diff window: act on the change at
    /// point, or with the region active on its added lines and its changes' removed rows.
    fn diff_update_change(&mut self, action: Action) -> Result<(), String> {
        let core = &*self.core;
        let (list, window, shown) = core.diff_shown().ok_or("No diff shown")?;
        let action = match (action, shown.staged()) {
            (Action::Stage, true) => return Err("Already staged".into()),
            (Action::Unstage, false) => return Err("Not staged".into()),
            (Action::Discard, true) => Action::Unstage,
            (action, _) => action,
        };
        let Row::File {
            root,
            repository,
            file,
            ..
        } = &shown.row
        else {
            unreachable!("a shown row is a file's");
        };
        let id = window.buffer();
        let buffer = core.buffers.get(id).expect("window buffer is live");
        let full = repository.join(file);
        let path = full.strip_prefix(root).unwrap_or(file).display();
        // Git sees the disk.
        if action == Action::Stage && buffer.modified() {
            return Err(format!("Save {path} first"));
        }
        // Hunks of an older text would build a wrong patch.
        if shown.request != 0 || shown.timer.is_some() {
            return Err("The diff is being worked out".into());
        }
        let (line, last) = core.diff_line(window);
        let mark = window.mark(&core.buffers).filter(|_| window.active());
        let region = mark.map(|mark| {
            let point = window.point(&core.buffers);
            let at = |i| buffer.line_col(i).expect("in the buffer");
            let ((start, _), (end, col)) = (at(point.min(mark)), at(point.max(mark)));
            start..if col == 0 && end > start {
                end
            } else {
                end + 1
            }
        });
        // Each change with the new lines chosen: the region's, else all of the change at point.
        let picks: Vec<(&Hunk, Range<usize>)> = match &region {
            Some(lines) => (shown.hunks.iter())
                .filter(|h| {
                    (h.new.start < lines.end && lines.start < h.new.end)
                        || lines.contains(&Shown::start(h, last))
                })
                .map(|h| {
                    let start = h.new.start.max(lines.start);
                    (h, start..h.new.end.min(lines.end).max(start))
                })
                .collect(),
            None => (shown.change_at((line, last)).map(|i| &shown.hunks[i]))
                .map(|h| (h, h.new.clone()))
                .into_iter()
                .collect(),
        };
        if picks.is_empty() {
            return Err("No change here".into());
        }
        let diff = core
            .buffers
            .get(list)
            .and_then(|b| b.diff_list())
            .expect("shown");
        let status = (diff.answer.iter().flatten())
            .find(|(r, _)| r == root)
            .and_then(|(_, s)| s.as_ref().ok()?.as_ref());
        let git_file = status.and_then(|s| s.files.iter().find(|f| f.path == *file));
        let state = git_file.and_then(|f| if shown.staged() { f.staged } else { f.unstaged });
        use GitChange::*;
        // No index entry under this path on one side, so the file goes whole.
        let whole = matches!(state, Some(Added | Deleted | Untracked | Renamed | Copied));
        let old = shown.old.as_ref().expect("shown");
        let new = buffer.text();
        let chars = |t: &Rope, r: &Range<usize>| {
            let to = |i: usize| t.line_to_char(i.min(t.len_lines()));
            to(r.start)..to(r.end)
        };
        let lines = |t: &Rope, r: Range<usize>| t.slice(chars(t, &r)).to_string();
        // Staging edits the index toward the new side; unstaging and discarding edit the new
        // side back, keeping the added lines not chosen.
        let edits: Vec<(Range<usize>, String)> = (picks.into_iter())
            .map(|(h, chosen)| match action {
                Action::Stage => (chars(old, &h.old), lines(new, chosen)),
                _ => {
                    let mut back = lines(old, h.old.clone());
                    let kept =
                        lines(new, h.new.start..chosen.start) + &lines(new, chosen.end..h.new.end);
                    // Only a text's last line goes without its newline.
                    if !back.is_empty() && !back.ends_with('\n') && !kept.is_empty() {
                        back.push('\n');
                    }
                    (chars(new, &h.new), back + &kept)
                }
            })
            .collect();
        let steps = match (action, whole) {
            (Action::Discard, _) => {
                let core = &mut *self.core;
                let buffer = core.buffers.get_mut(id).expect("live");
                if buffer.read_only {
                    return Err("Buffer is read-only".into());
                }
                let point = core.windows.window.point(&core.buffers);
                let buffer = core.buffers.get_mut(id).expect("live");
                buffer.undo_boundary(point);
                buffer.replace(&edits).expect("the hunks are of this text");
                let point = core.windows.window.point(&core.buffers);
                core.buffers.get_mut(id).expect("live").undo_boundary(point);
                core.windows.window.set_active(false);
                return Ok(());
            }
            (Action::Stage, true) => vec![(GitStep::Stage, vec![file.clone()])],
            (_, true) => {
                let old_path = git_file.and_then(|f| f.old_path.clone());
                vec![(
                    GitStep::Unstage,
                    std::iter::once(file.clone()).chain(old_path).collect(),
                )]
            }
            (_, false) => {
                let base = if action == Action::Stage { old } else { new };
                let mut edited = base.clone();
                for (range, text) in edits.iter().rev() {
                    edited.remove(range.clone());
                    edited.insert(range.start, text);
                }
                let patch = crate::buffer::patch(file, &base.to_string(), &edited.to_string());
                vec![(GitStep::Apply(patch), Vec::new())]
            }
        };
        let repository = repository.clone();
        self.core.windows.window.set_active(false);
        self.core
            .buffers
            .get_mut(list)
            .and_then(|b| b.diff_list_mut())
            .expect("a list")
            .updating = true;
        self.effects.push(Effect::File(FileOp::GitUpdate {
            list,
            repository,
            steps,
        }));
        Ok(())
    }

    /// `diff-copy-removed`: puts the removed lines of the change at point on the kill ring.
    pub(crate) fn diff_copy_removed(&mut self) -> Result<(), String> {
        let core = &*self.core;
        let (_, window, shown) = core.diff_shown().ok_or("No diff shown")?;
        let hunk = (shown.change_at(core.diff_line(window)))
            .map(|i| shown.hunks[i].clone())
            .filter(|h| !h.old.is_empty())
            .ok_or("No removed lines here")?;
        let old = shown.old.as_ref().expect("shown");
        let end = old.line_to_char(hunk.old.end.min(old.len_lines()));
        let mut text = old.slice(old.line_to_char(hunk.old.start)..end).to_string();
        if !text.ends_with('\n') {
            text.push('\n');
        }
        let n = hunk.old.len();
        self.copy_text(text);
        let lines = if n == 1 { "line" } else { "lines" };
        self.core.echo(format!("Copied {n} removed {lines}"));
        Ok(())
    }

    /// `diff-visit`: quits the view and shows the diff's file at point in the restored layout;
    /// with the region active, only deactivates it.
    pub(crate) fn diff_visit(&mut self) -> Result<(), String> {
        if self.core.windows.window.active() {
            self.core.windows.window.set_active(false);
            return Ok(());
        }
        let core = &*self.core;
        let visit = core.diff_shown().map(|(_, window, shown)| {
            let buffer = core
                .buffers
                .get(window.buffer())
                .expect("window buffer is live");
            let at = buffer
                .line_col(window.point(&core.buffers))
                .unwrap_or_default();
            let Row::File {
                repository, file, ..
            } = &shown.row
            else {
                unreachable!("a shown row is a file's");
            };
            (shown.fixed(), window.buffer(), repository.join(file), at)
        });
        self.diff_quit()?;
        let Some((staged, buffer, path, (line, col))) = visit else {
            return Ok(());
        };
        match staged {
            false => self.core.switch_to(buffer),
            true => {
                self.find_path(path);
                if let Some(pending) = &mut self.core.pending_visit {
                    pending.destination = Some((line, col));
                    return Ok(());
                }
            }
        }
        self.core.goto_line_col(line, col);
        Ok(())
    }

    /// `diff-quit`: restores the layout the view replaced and kills the list.
    pub(crate) fn diff_quit(&mut self) -> Result<(), String> {
        self.outside_minibuffer("Can't quit the diff view from the minibuffer")?;
        self.core.leave_diff_view().ok_or("No diff view".into())
    }
}

impl Core {
    /// Restores the layout the diff view replaced and kills the list; `None` outside the view.
    pub(crate) fn leave_diff_view(&mut self) -> Option<()> {
        let saved = self.windows.saved.take()?;
        if let Some(list) = self.diff_list_here() {
            self.drop_shown(list);
        }
        let mut view = std::mem::replace(&mut self.windows, *saved);
        view.windows_mut()
            .for_each(|w| w.release(&mut self.buffers));
        self.pending_visit = None;
        if let Some(list) = self.diff_list_here() {
            self.kill_buffer(list);
        }
        self.buffers.raise(self.windows.window.buffer());
        Some(())
    }

    /// Before buffer `id` is shown in the selected window: quits the diff view, a temporary
    /// layout, unless `id` is already there or is the list; `false` when quitting killed `id`.
    pub(crate) fn leave_diff_view_for(&mut self, id: BufferId) -> bool {
        if self.windows.saved.is_some()
            && self.windows.window.buffer() != id
            && self.diff_list_here() != Some(id)
        {
            self.leave_diff_view();
        }
        self.buffers.get(id).is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::buffer::Decoration;
    use crate::command::Args;
    use crate::harness::Harness;
    use crate::project::Root;
    use crate::{ChangedFile, GitFile, Msg};
    use std::ops::Not;

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
                "▾ a  ⎇ main",
                "  ▾ Staged     1",
                "      M  src/x.rs",
                "  ▾ Changes    2",
                "      M  src/x.rs",
                "      ?  notes",
                "",
                "▾ b  ⎇ topic",
                "  ▾ Changes    1",
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
        assert!(h.core.windows.saved.is_some());
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
        assert!(h.core.windows.saved.is_some(), "killing stays in the view");
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

    /// The harness over `f` and `g`, `f` current in the top of two windows, and that layout.
    fn split_over_files() -> (Harness, Vec<(WindowId, String)>, WindowId) {
        let mut h = Harness::new(&[("f", "x"), ("g", "y")]);
        h.read_reply("h", Some(b"z"));
        h.open("g").open("f").keys("C-x 2");
        let before = shown(&h);
        let selected = h.core.windows.selected;
        (h, before, selected)
    }

    #[test]
    fn showing_another_buffer_quits_the_view_from_either_window() {
        let (mut h, before, selected) = split_over_files();
        for from in ["", "C-x o "] {
            for (keys, name) in [
                ("C-x b g RET", "g"),
                ("C-x C-f C-a C-k /aiditor-test/g RET", "g"),
                ("C-x C-f C-a C-k /aiditor-test/h RET", "h"),
            ] {
                h.call("diff-status", Args::default());
                h.keys(&format!("{from}{keys}"));
                assert!(
                    h.core.windows.saved.is_none(),
                    "{from}{keys} {:?} {:?}",
                    h.core.message(),
                    shown(&h)
                );
                assert!(h.core.buffers.find_here("*Diff*<main>").is_none());
                let mut after = before.clone();
                after[0].1 = name.into();
                assert_eq!(shown(&h), after, "{from}{keys}");
                assert_eq!(h.core.windows.selected, selected);
                h.keys("C-x b f RET");
            }
        }
        // A link or grep hit followed, from the view, lands in the restored window.
        h.call("diff-status", Args::default());
        let g = h.path("g");
        h.ctx().find_file_at(&g, 0, 1).unwrap();
        assert!(h.core.windows.saved.is_none());
        assert_eq!((shown(&h)[0].1.as_str(), h.point()), ("g", 1));
        // A jump back to another buffer too.
        h.call("diff-status", Args::default());
        h.call("xref-go-back", Args::default());
        assert!(h.core.windows.saved.is_none(), "{:?}", h.core.message());
        assert_eq!(shown(&h)[0].1, "f");
    }

    #[test]
    fn changing_the_window_tree_quits_the_view_first() {
        let cases = [("C-x 0", 1), ("C-x 1", 1), ("C-x 2", 3), ("C-x 3", 3)];
        for ((keys, windows), from) in cases.into_iter().flat_map(|c| [(c, ""), (c, "C-x o ")]) {
            let (mut h, before, selected) = split_over_files();
            h.call("diff-status", Args::default());
            h.keys(&format!("{from}{keys}"));
            assert!(h.core.windows.saved.is_none(), "{keys}");
            assert_eq!(h.core.windows.tree.leaves().len(), windows, "{keys}");
            let names: Vec<_> = shown(&h).into_iter().map(|(_, name)| name).collect();
            assert!(names.iter().all(|n| n == "f"), "{keys} {names:?}");
            if windows == 3 {
                assert_eq!(h.core.windows.selected, selected, "{keys} {before:?}");
            }
        }
    }

    #[test]
    fn showing_a_buffer_in_another_window_quits_the_view() {
        let mut h = Harness::new(&[("f", "x\n")]);
        h.open("f").keys("y");
        h.diffs.insert(h.path("f"), Ok("-x\n+yx\n".into()));
        h.call("diff-status", Args::default());
        h.keys("C-x o M-x diff-buffer-with-file RET");
        assert!(h.core.windows.saved.is_none());
        let names: Vec<_> = shown(&h).into_iter().map(|(_, name)| name).collect();
        assert_eq!(names, ["f", "*Diff*"]);
    }

    #[test]
    fn working_in_the_view_keeps_it_open() {
        let mut h = inline(false, true, Some("a\nB\n"));
        answer_diff(&mut h, "a\nb\n", "a\nB\n");
        let view = shown(&h);
        h.keys("C-x o z C-x C-s C-x o C-x o C-c n C-x o n C-x b *Diff*<main> RET");
        assert!(h.core.windows.saved.is_some(), "{:?}", h.core.message());
        assert_eq!(h.writes.last().unwrap().0, h.path("a/x.rs"));
        assert_eq!(shown(&h), view);
        // The list again from the diff window shows it there too, in the view.
        h.keys("C-x o C-x b *Diff*<main> RET");
        assert!(h.core.windows.saved.is_some());
    }

    #[test]
    fn a_buffer_the_quit_kills_is_not_shown() {
        let mut h = inline(true, false, Some("a\n"));
        answer_diff(&mut h, "b\n", "a\n");
        let index = right(&h).name.clone();
        h.keys(&format!("C-x b {index} RET"));
        assert!(h.core.windows.saved.is_none());
        assert!(h.core.buffers.find_here(&index).is_none());
        assert_eq!(shown(&h).len(), 1);
    }

    #[test]
    fn the_workspace_list_from_the_view_quits_it() {
        let mut h = Harness::new(&[]);
        h.call("workspace-list", Args::default());
        h.call("diff-status", Args::default());
        h.call("workspace-list", Args::default());
        assert!(h.core.windows.saved.is_none());
        assert_eq!(shown(&h)[0].1, "*Workspaces*");
    }

    /// The view over root `a`, its own repository on `main`, with `x.rs` `staged` and/or
    /// `changed`, on disk as `disk` (`None`: deleted); point on its first row.
    fn inline(staged: bool, changed: bool, disk: Option<&str>) -> Harness {
        inline_file("x.rs", staged, changed, disk)
    }

    fn inline_file(name: &str, staged: bool, changed: bool, disk: Option<&str>) -> Harness {
        let mut h = Harness::new(&[]);
        h.read_reply(&format!("a/{name}"), disk.map(str::as_bytes));
        let project = &mut h.core.projects[0].project;
        project.primary.path = h.dir.join("a");
        project.roots = Vec::new();
        h.call("diff-status", Args::default());
        let a = h.dir.join("a");
        let change = |on: bool| on.then_some(GitChange::Modified);
        let status = GitStatus {
            repository: a.clone(),
            branch: "main".into(),
            files: vec![file(name, change(staged), change(changed))],
        };
        h.send(Msg::GitStatus(vec![(a, Ok(Some(status)))]));
        // Laid out, as the app's windows are before an answer arrives.
        h.frame(1440., 900.);
        h
    }

    /// Answers the newest `GitDiff` asked with `old` and `new`, returning its sides.
    fn answer_diff(h: &mut Harness, old: &str, new: &str) -> (GitSide, GitSide) {
        let i = (h.effects.iter())
            .rposition(|e| matches!(e, Effect::File(FileOp::GitDiff { .. })))
            .expect("a diff asked");
        let Effect::File(FileOp::GitDiff {
            request,
            old: from,
            new: to,
            ..
        }) = h.effects.remove(i)
        else {
            unreachable!()
        };
        let result = Ok(FileDiff {
            old: old.into(),
            new: new.into(),
            hunks: crate::buffer::hunks(old, new),
        });
        h.send(Msg::GitDiffed { request, result });
        (from, to)
    }

    /// The right window's lines after their signs, `-`, `+` or blank, each signed row on its
    /// face's background and a removed row unnumbered.
    fn drawn(h: &mut Harness) -> Vec<String> {
        let frame = h.frame(1440., 900.);
        let window = &frame.windows[1];
        let gutter = window.gutter.as_ref().expect("a gutter");
        let rows = (window.display.lines.iter()).zip(gutter.rows.iter().zip(&gutter.signs));
        rows.map(|(l, ((label, _), (sign, _)))| {
            let text: String = l.spans.iter().map(|s| s.text.as_str()).collect();
            let face = match sign.as_str() {
                "+" => FaceName::DiffAdded,
                "-" => FaceName::DiffRemoved,
                _ => return format!("  {text}"),
            };
            assert!(
                l.decorations.contains(&Decoration::Background(face)),
                "{text}"
            );
            assert!(
                l.diff.is_none() || label.is_empty(),
                "{text} numbered {label}"
            );
            format!("{sign} {text}")
        })
        .collect()
    }

    fn mode_line(h: &mut Harness) -> String {
        h.frame(1440., 900.).windows[1].display.mode_line.text()
    }

    fn right(h: &Harness) -> &crate::buffer::Buffer {
        let id = (h.core.windows.others.values())
            .chain([&h.core.windows.window])
            .find(|w| (h.core.buffers.get(w.buffer())).is_some_and(|b| b.diff_list().is_none()))
            .unwrap()
            .buffer();
        h.core.buffers.get(id).unwrap()
    }

    #[test]
    fn a_change_shows_in_place_over_the_files_buffer() {
        let mut h = inline(false, true, Some("a\nB\nc\nd\n"));
        let sides = answer_diff(&mut h, "a\nb\nc\n", "a\nB\nc\nd\n");
        assert_eq!(sides, (GitSide::Index, GitSide::WorkingTree));
        assert_eq!(right(&h).path(), Some(h.path("a/x.rs").as_path()));
        assert_eq!(drawn(&mut h), ["  a", "- b", "+ B", "  c", "+ d", "  "]);
        let bar = mode_line(&mut h);
        assert!(bar.contains("+2 −1 │ index → working tree"), "{bar}");
        assert!(
            bar.contains("change 1/2"),
            "opens on the first change: {bar}"
        );
        h.keys("n");
        assert!(mode_line(&mut h).contains("change 2/2"));
        h.keys("C-x o M-<");
        assert!(mode_line(&mut h).contains("2 changes"));
    }

    #[test]
    fn typing_edits_the_file_and_the_rows_follow() {
        let mut h = inline(false, true, Some("a\nb\n"));
        answer_diff(&mut h, "a\nb\n", "a\nb\n");
        assert_eq!(drawn(&mut h), ["  a", "  b", "  "]);
        h.keys("C-x o C-n C-k C-k");
        assert_eq!(
            drawn(&mut h),
            ["  a", "  "],
            "the old answer until the pause"
        );
        h.advance(REDIFF);
        assert_eq!(drawn(&mut h), ["  a", "- b", "  "]);
        h.send(Msg::LinesDiffed {
            request: 1,
            hunks: Vec::new(),
        });
        assert_eq!(
            drawn(&mut h),
            ["  a", "- b", "  "],
            "a stale answer is dropped"
        );
        assert!(right(&h).modified());
        h.keys("C-x C-s");
        assert_eq!(h.writes.last().unwrap(), &(h.path("a/x.rs"), "a\n".into()));
    }

    #[test]
    fn a_reverted_buffer_is_diffed_again() {
        let mut h = inline(false, true, Some("a\n"));
        answer_diff(&mut h, "a\n", "a\n");
        h.read_reply("a/x.rs", Some(b"a\nnew\n"));
        h.prepare_revert("a/x.rs", Some(b"a\nnew\n"));
        let path = h.path("a/x.rs");
        h.send(Msg::FilesChanged {
            paths: vec![(path, crate::Disk::File)],
            rescan: false,
        });
        h.advance(REDIFF);
        assert_eq!(drawn(&mut h), ["  a", "+ new", "  "]);
    }

    #[test]
    fn staged_and_changed_are_two_diffs_and_staged_is_read_only() {
        let mut h = inline(true, true, Some("c\n"));
        let sides = answer_diff(&mut h, "a\n", "b\n");
        assert_eq!(sides, (GitSide::Tree("HEAD".into()), GitSide::Index));
        assert_eq!(right(&h).name, "x.rs (index)");
        assert_eq!(drawn(&mut h), ["- a", "+ b", "  "]);
        assert!(mode_line(&mut h).contains("HEAD → index"));
        h.keys("C-x o z");
        assert_eq!(h.core.message(), Some("Buffer is read-only"));
        h.keys("C-x o f");
        let sides = answer_diff(&mut h, "b\n", "c\n");
        assert_eq!(sides, (GitSide::Index, GitSide::WorkingTree));
        assert_eq!(right(&h).path(), Some(h.path("a/x.rs").as_path()));
        assert!(
            h.core.buffers.find_here("x.rs (index)").is_none(),
            "the index buffer goes with its diff"
        );
        let old = |h: &mut Harness| {
            (h.core.buffers.iter_mut())
                .filter(|(_, b)| b.name.starts_with(" *diff old*"))
                .count()
        };
        assert_eq!(old(&mut h), 1, "so does the old side's");
        h.keys("f");
        assert_eq!(h.core.message(), Some("No next file"));
        h.keys("b b");
        assert_eq!(h.core.message(), Some("No previous file"));
    }

    /// The right window's lines as runs of text and colour.
    fn colours(h: &mut Harness) -> Vec<Vec<(String, Option<u32>)>> {
        let frame = h.frame(1440., 900.);
        (frame.windows[1].display.lines.iter())
            .map(|l| {
                (l.spans.iter())
                    .map(|s| (s.text.clone(), s.face.fg))
                    .collect()
            })
            .collect()
    }

    fn syntax(h: &Harness, capture: &str) -> Option<u32> {
        h.core.settings().theme.syntax[capture].fg
    }

    #[test]
    fn removed_rows_are_highlighted_from_the_old_text() {
        let old = "/* old\nfn gone() {}\n*/\nfn kept() {}\n";
        let new = "fn gone() {} // new\nfn kept() {}\n";
        let mut h = inline(false, true, Some(new));
        answer_diff(&mut h, old, new);
        let (comment, keyword) = (syntax(&h, "comment"), syntax(&h, "keyword"));
        let removed = h.core.settings().theme.face(FaceName::DiffRemoved).fg;
        let lines = colours(&mut h);
        assert_eq!(
            lines[1],
            [("fn gone() {}".into(), comment)],
            "inside the old comment"
        );
        assert_eq!(lines[3][0], ("fn".into(), keyword), "the same text, added");
        assert_eq!(lines[4][0], ("fn".into(), keyword), "unchanged");
        assert!(lines[0].iter().all(|(_, fg)| *fg == comment));
        assert_ne!(comment, removed);
        let theme = "[themes.default.syntax]\ncomment = { fg = 7 }";
        h.send(Msg::SettingsLoaded(Ok(Some(theme.into()))));
        assert_eq!(
            colours(&mut h)[1],
            [("fn gone() {}".into(), Some(7))],
            "the theme followed"
        );
    }

    #[test]
    fn a_staged_side_takes_the_files_grammar() {
        let mut h = inline(true, false, None);
        answer_diff(&mut h, "", "fn f() {}\n");
        assert_eq!(colours(&mut h)[0][0], ("fn".into(), syntax(&h, "keyword")));
    }

    #[test]
    fn a_language_without_a_grammar_is_plain_on_the_diff_backgrounds() {
        let mut h = inline_file("x.zzz", false, true, Some("fn b\n"));
        answer_diff(&mut h, "fn a\n", "fn b\n");
        assert_eq!(
            colours(&mut h)[..2],
            [vec![("fn a".into(), None)], vec![("fn b".into(), None)]]
        );
        assert_eq!(drawn(&mut h)[..2], ["- fn a", "+ fn b"]);
    }

    #[test]
    fn a_large_old_side_is_coloured_when_its_parse_arrives() {
        let line = "fn f() {}\n";
        let old = line.repeat(crate::syntax::PARSE_CUT / line.len() + 1);
        let mut h = inline(false, true, Some(""));
        answer_diff(&mut h, &old, "");
        assert_eq!(colours(&mut h)[0], [("fn f() {}".into(), None)]);
        assert_eq!(h.parses.len(), 1, "the old side parsed off the core thread");
        h.parse();
        assert_eq!(colours(&mut h)[0][0], ("fn".into(), syntax(&h, "keyword")));
    }

    #[test]
    fn next_and_previous_change_stop_at_each_and_say_so_at_the_ends() {
        let mut h = inline(false, true, Some("a\nB\nc\nd\n"));
        answer_diff(&mut h, "a\nb\nc\n", "a\nB\nc\nd\n");
        h.keys("C-x o");
        let line = |h: &Harness| h.core.current().line_col(h.point()).unwrap().0;
        assert_eq!(line(&h), 1, "the diff opens on its first change");
        h.keys("C-c n");
        assert_eq!(line(&h), 3);
        h.keys("C-c n");
        assert_eq!(h.core.message(), Some("No next change"));
        assert_eq!(line(&h), 3);
        h.keys("C-c p C-c p");
        assert_eq!(h.core.message(), Some("No previous change"));
        assert_eq!(line(&h), 1);
    }

    #[test]
    fn copying_a_changes_removed_lines() {
        let mut h = inline(false, true, Some("a\nB\nC\n"));
        answer_diff(&mut h, "a\nb\nc", "a\nB\nC\n");
        h.keys("w");
        assert_eq!(h.core.message(), Some("Copied 2 removed lines"));
        assert_eq!(h.clipboard.as_deref(), Some("b\nc\n"));
        h.keys("C-x o M-< C-c w");
        assert_eq!(h.core.message(), Some("No removed lines here"));
    }

    #[test]
    fn c_g_quits_to_the_file_at_point_keeping_unsaved_edits() {
        let mut h = inline(false, true, Some("a\nb\nc\n"));
        answer_diff(&mut h, "a\nb\nc\n", "a\nb\nc\n");
        h.keys("C-x o C-n C-n x");
        h.keys("C-SPC C-g");
        assert!(h.core.windows.saved.is_some(), "C-g first drops the region");
        h.keys("C-g");
        assert!(h.core.windows.saved.is_none());
        assert_eq!(shown(&h).len(), 1);
        assert_eq!(h.core.current().path(), Some(h.path("a/x.rs").as_path()));
        assert!(h.core.current().modified());
        assert_eq!(h.core.current().line_col(h.point()).unwrap(), (2, 1));
    }

    #[test]
    fn c_g_from_a_staged_diff_visits_the_file_at_the_line() {
        let mut h = inline(true, false, Some("a\nb\nc\n"));
        answer_diff(&mut h, "a\n", "a\nb\nc\n");
        h.keys("C-x o C-n C-g");
        assert_eq!(h.core.current().path(), Some(h.path("a/x.rs").as_path()));
        assert_eq!(h.core.current().line_col(h.point()).unwrap(), (2, 0));
        assert!(h.core.buffers.find_here("x.rs (index)").is_none());
    }

    #[test]
    fn untracked_deleted_and_binary_files() {
        let mut h = inline(false, true, Some("x\n"));
        answer_diff(&mut h, "", "x\n");
        assert_eq!(drawn(&mut h), ["+ x", "  "], "untracked: all added");
        let mut h = inline(false, true, None);
        answer_diff(&mut h, "a\n", "");
        assert_eq!(drawn(&mut h), ["- a", "  "], "deleted: all removed");
        let mut h = inline(false, true, Some("\0"));
        let i = (h.effects.iter())
            .rposition(|e| matches!(e, Effect::File(FileOp::GitDiff { .. })))
            .unwrap();
        let Effect::File(FileOp::GitDiff { request, .. }) = h.effects.remove(i) else {
            unreachable!()
        };
        let result = Err("x.rs: binary file".into());
        h.send(Msg::GitDiffed { request, result });
        assert_eq!(right(&h).text().to_string(), "x.rs: binary file");
    }

    #[test]
    fn c_g_in_the_minibuffer_still_aborts_it() {
        let mut h = inline(false, true, Some("a\n"));
        h.keys("M-x C-g");
        assert!(h.core.prompts.minibuffer.is_none());
        assert!(h.core.windows.saved.is_some(), "the view stays");
    }

    #[test]
    fn a_removal_ending_a_file_without_a_newline_is_a_change_on_its_last_line() {
        let mut h = inline(false, true, Some("a\nb"));
        answer_diff(&mut h, "a\nb\nc", "a\nb");
        // `similar` counts the unterminated `b` as a changed line.
        assert_eq!(drawn(&mut h), ["  a", "- b", "- c", "+ b"]);
        h.keys("C-x o C-c n");
        assert!(mode_line(&mut h).contains("change 1/1"));
        h.keys("C-c n");
        assert_eq!(h.core.message(), Some("No next change"));
        h.keys("C-c w");
        assert_eq!(h.clipboard.as_deref(), Some("b\nc\n"));
    }

    #[test]
    fn the_views_keys_work_in_either_window() {
        let mut h = inline(true, true, Some("a\nB\n"));
        answer_diff(&mut h, "a\n", "a\nb\n");
        h.keys("C-x o C-c f");
        answer_diff(&mut h, "a\nb\n", "a\nB\n");
        assert_eq!(right(&h).path(), Some(h.path("a/x.rs").as_path()));
        h.keys("C-c n C-c p");
        assert_eq!(h.core.message(), Some("No previous change"));
        h.keys("C-c w");
        assert_eq!(h.clipboard.as_deref(), Some("b\n"));
        h.keys("C-c b");
        assert_eq!(h.core.message(), None, "back on the staged row");
        answer_diff(&mut h, "a\n", "a\nb\n");
        h.keys("C-x o p");
        assert_eq!(h.core.message(), Some("No previous change"));
        h.keys("C-g");
        assert!(h.core.windows.saved.is_none(), "C-g in the list quits too");
    }

    #[test]
    fn each_diff_row_piece_has_its_face() {
        let mut h = inline(false, true, Some("a\nB\n"));
        answer_diff(&mut h, "a\nb\n", "a\nB\n");
        h.keys("C-x o");
        let theme = h.core.settings().theme.clone();
        let sign = |name| Face {
            bg: None,
            ..theme.face(name)
        };
        let frame = h.frame(1440., 900.);
        let window = &frame.windows[1];
        let gutter = window.gutter.as_ref().expect("a gutter");
        let signs: Vec<_> = gutter.signs.iter().map(|(s, f)| (s.as_str(), *f)).collect();
        let blank = ("", Face::default());
        let removed = ("-", sign(FaceName::DiffRemoved));
        let added = ("+", sign(FaceName::DiffAdded));
        assert_eq!(signs, [blank, removed, added, blank]);
        assert_eq!(gutter.rows[1].0, "", "a removed row has no line number");
        let lines = &window.display.lines;
        assert_eq!(
            lines[1].decorations,
            [Decoration::Background(FaceName::DiffRemoved)]
        );
        // Point's line keeps the added background over hl-line.
        assert_eq!(
            lines[2].decorations,
            [
                Decoration::Background(FaceName::HlLine),
                Decoration::Background(FaceName::DiffAdded)
            ]
        );
        let plain = crate::display::gutter_width(3, 8.);
        assert_eq!(gutter.width, plain + crate::display::sign_width(8.));
        let bar = window.display.mode_line.text();
        assert!(bar.contains("change 1/1 │ 2:0"), "{bar}");
        let hints = &frame.key_hints;
        assert_eq!(hints[0], ("C-c n/p".into(), "change"), "{hints:?}");
        assert!(hints.contains(&("C-c q".into(), "quit")), "{hints:?}");
        assert!(
            hints.iter().all(|(_, a)| *a != "fold"),
            "TAB isn't bound here"
        );
    }

    /// The face of the first span reading `text` in the list's display line `line`.
    fn list_face(h: &mut Harness, line: usize, text: &str) -> Face {
        let frame = h.frame(1440., 900.);
        let spans = &frame.windows[0].display.lines[line].spans;
        (spans.iter().find(|s| s.text == text))
            .unwrap_or_else(|| panic!("no {text:?} in {spans:?}"))
            .face
    }

    #[test]
    fn each_list_row_piece_has_its_face() {
        let mut h = view();
        let answer = answer(&h, &[]);
        h.send(answer);
        let theme = h.core.settings().theme.clone();
        let fg = |name| theme.face(name).fg;
        assert_eq!(list_face(&mut h, 0, "▾ ").fg, fg(FaceName::ModeLineDim));
        assert!(list_face(&mut h, 0, "a").bold);
        assert_eq!(list_face(&mut h, 0, "⎇ main").fg, fg(FaceName::VcsBranch));
        let group = list_face(&mut h, 1, "Staged");
        assert_eq!((group.fg, group.bold), (fg(FaceName::DiffListGroup), true));
        assert_eq!(
            list_face(&mut h, 1, "  1").fg,
            fg(FaceName::SessionListCount)
        );
        assert_eq!(list_face(&mut h, 2, "M").fg, fg(FaceName::DiffFileModified));
        assert_eq!(list_face(&mut h, 2, "src/").fg, fg(FaceName::ModeLineDim));
        assert_eq!(list_face(&mut h, 2, "x.rs").fg, None);
        assert_eq!(
            list_face(&mut h, 5, "?").fg,
            fg(FaceName::DiffFileUntracked)
        );
        assert_eq!(
            list_face(&mut h, 12, "    No changes").fg,
            fg(FaceName::Annotation)
        );
        // The shown file's row stays marked from the diff window.
        h.keys("C-x o");
        let frame = h.frame(1440., 900.);
        let hl = Decoration::Background(FaceName::HlLine);
        assert!(frame.windows[0].display.lines[2].decorations.contains(&hl));
    }

    #[test]
    fn tab_folds_a_root_until_unfolded_across_refreshes() {
        let mut h = view();
        let status = answer(&h, &[]);
        h.send(status);
        h.keys("C-p C-p TAB");
        let text = h.text();
        let lines: Vec<_> = text.lines().collect();
        assert_eq!(
            lines[..4],
            ["▸ a  ⎇ main", "", "▾ b  ⎇ topic", "  ▾ Changes    1"]
        );
        assert_eq!(h.point(), 0, "point on the heading");
        h.keys("g");
        let status = answer(&h, &[]);
        h.send(status);
        assert!(h.text().starts_with("▸ a  ⎇ main\n\n▾ b"), "kept folded");
        h.keys("TAB");
        assert!(h.text().starts_with("▾ a  ⎇ main\n  ▾ Staged"));
        h.keys("C-n C-n C-n C-n C-n C-n C-n C-n C-n C-n C-n");
        assert!(
            h.text()
                .lines()
                .nth(11)
                .is_some_and(|l| l.starts_with("  c"))
        );
        h.keys("TAB");
        assert_eq!(h.core.message(), Some("Nothing to fold"));
    }

    fn point_line(h: &Harness) -> usize {
        h.text()
            .chars()
            .take(h.point())
            .filter(|&c| c == '\n')
            .count()
    }

    #[test]
    fn tab_folds_one_group_until_unfolded_across_refreshes() {
        let mut h = view();
        let status = answer(&h, &[]);
        h.send(status);
        h.keys("C-n C-n TAB");
        let folded = [
            "▾ a  ⎇ main",
            "  ▾ Staged     1",
            "      M  src/x.rs",
            "  ▸ Changes    2",
            "",
        ];
        assert_eq!(h.text().lines().take(5).collect::<Vec<_>>(), folded);
        assert_eq!(point_line(&h), 3, "point on the group's heading");
        h.keys("g");
        let status = answer(&h, &[]);
        h.send(status);
        assert_eq!(
            h.text().lines().take(5).collect::<Vec<_>>(),
            folded,
            "kept folded"
        );
        h.keys("C-p C-p C-p TAB TAB");
        assert_eq!(
            h.text().lines().take(5).collect::<Vec<_>>(),
            folded,
            "kept under a root fold"
        );
        h.keys("C-n C-n C-n TAB");
        assert!(
            h.text()
                .contains("  ▾ Changes    2\n      M  src/x.rs\n      ?  notes")
        );
    }

    #[test]
    fn group_counts_line_up_up_to_three_digits() {
        let mut h = view();
        let many: Vec<_> = (0..120)
            .map(|i| file(&format!("a/f{i}"), None, Some(GitChange::Untracked)))
            .collect();
        let status = answer(&h, &many);
        h.send(status);
        let ends: Vec<_> = (h.text().lines())
            .filter(|l| l.contains("Staged") || l.contains("Changes"))
            .map(|l| (l.chars().count(), l.to_string()))
            .collect();
        assert!(ends.iter().any(|(_, l)| l.ends_with("122")), "{ends:?}");
        assert!(ends.iter().all(|(n, _)| *n == ends[0].0), "{ends:?}");
    }

    #[test]
    fn a_long_path_is_cut_from_the_left_to_fit() {
        let mut h = view();
        let long = file("a/deep/nested/dir/name.rs", None, Some(GitChange::Added));
        let status = answer(&h, &[long]);
        h.send(status);
        let frame = h.frame(600., 900.);
        let window = &frame.windows[0];
        let cols = ((window.rect.width - 24.) / 8.) as usize;
        let rows: Vec<String> = (window.display.lines.iter())
            .map(|l| l.spans.iter().map(|s| s.text.as_str()).collect())
            .collect();
        let row = rows
            .iter()
            .find(|r| r.ends_with("name.rs"))
            .expect("the file's row");
        assert!(row.starts_with("      A  …"), "{row}");
        assert_eq!(row.chars().count(), cols, "{row}");
        let short = rows
            .iter()
            .find(|r| r.ends_with("x.rs"))
            .expect("a short row");
        assert_eq!(short, "      M  src/x.rs");
    }

    #[test]
    fn only_the_first_answer_moves_point_to_the_first_change() {
        let mut h = inline(false, true, Some("a\nb\nC\n"));
        answer_diff(&mut h, "a\nb\nc\n", "a\nb\nC\n");
        h.keys("C-x o");
        let line = |h: &Harness| h.core.current().line_col(h.point()).unwrap().0;
        assert_eq!(line(&h), 2, "on the first change");
        h.keys("M-< x");
        h.advance(REDIFF);
        assert_eq!(drawn(&mut h)[..2], ["- a", "+ xa"], "re-diffed");
        assert_eq!(line(&h), 0, "a re-diff leaves point where editing put it");
    }

    #[test]
    fn a_staged_diff_is_numbered_and_names_its_branch() {
        let mut h = inline(true, false, None);
        let numbers = Some("line_numbers = \"absolute\"");
        h.core.settings = crate::settings::Settings::parse(numbers).unwrap();
        answer_diff(&mut h, "a\n", "a\nb\n");
        let frame = h.frame(1440., 900.);
        let window = &frame.windows[1];
        let labels: Vec<_> = (window.gutter.as_ref().expect("a gutter").rows.iter())
            .map(|(l, _)| l.as_str())
            .collect();
        assert_eq!(labels, ["1", "2", "3"]);
        let bar = window.display.mode_line.text();
        assert!(bar.contains("⎇ main │ change 1/1"), "{bar}");
    }

    type Update = (BufferId, PathBuf, Vec<(GitStep, Vec<PathBuf>)>);

    /// The newest `GitUpdate` asked, taken: its list, repository and steps.
    fn update(h: &mut Harness) -> Option<Update> {
        let i = (h.effects.iter())
            .rposition(|e| matches!(e, Effect::File(FileOp::GitUpdate { .. })))?;
        let Effect::File(FileOp::GitUpdate {
            list,
            repository,
            steps,
        }) = h.effects.remove(i)
        else {
            unreachable!()
        };
        Some((list, repository, steps))
    }

    /// Fails list `list`'s update, so the next one runs.
    fn fail(h: &mut Harness, list: BufferId) {
        h.send(Msg::GitUpdated {
            list,
            result: Err("failed".into()),
        });
    }

    fn on(h: &Harness) -> String {
        let line = h.core.current().line_col(h.point()).unwrap().0;
        h.text().lines().nth(line).unwrap().to_string()
    }

    fn statuses_asked(h: &Harness) -> usize {
        (h.effects.iter())
            .filter(|e| matches!(e, Effect::File(FileOp::GitStatus(_))))
            .count()
    }

    fn paths(names: &[&str]) -> Vec<PathBuf> {
        names.iter().map(PathBuf::from).collect()
    }

    #[test]
    fn staging_a_file_refreshes_and_point_walks_down_changes() {
        let mut h = view();
        let first = answer(&h, &[]);
        h.send(first);
        h.keys("C-n C-n");
        assert_eq!(on(&h), "      M  src/x.rs");
        h.keys("s");
        let (list, repository, steps) = update(&mut h).expect("asked");
        assert_eq!(repository, h.dir);
        assert_eq!(steps, [(GitStep::Stage, paths(&["a/src/x.rs"]))]);
        let asked = statuses_asked(&h);
        h.send(Msg::GitUpdated {
            list,
            result: Ok(()),
        });
        assert_eq!(statuses_asked(&h), asked + 1, "refreshed");
        // x.rs is now staged only, so its row under Changes left.
        let Msg::GitStatus(mut roots) = answer(&h, &[]) else {
            unreachable!()
        };
        let Ok(Some(a)) = &mut roots[0].1 else {
            unreachable!()
        };
        a.files[0].unstaged = None;
        h.send(Msg::GitStatus(roots));
        assert!(
            h.text()
                .contains("  ▾ Staged     1\n      M  src/x.rs\n  ▾ Changes    1\n")
        );
        assert_eq!(on(&h), "      ?  notes", "the next file");
        h.keys("u");
        assert_eq!(update(&mut h), None, "nothing staged here");
        assert_eq!(h.core.message(), Some("Nothing to unstage"));
        h.keys("M-< C-n C-n");
        assert_eq!(on(&h), "      M  src/x.rs");
        h.keys("u");
        let (_, _, steps) = update(&mut h).expect("asked");
        assert_eq!(steps, [(GitStep::Unstage, paths(&["a/src/x.rs"]))]);
    }

    #[test]
    fn point_whose_row_left_with_nothing_after_goes_before() {
        let mut h = view();
        let first = answer(&h, &[]);
        h.send(first);
        h.keys("C-n C-n C-n");
        assert_eq!(on(&h), "      ?  notes");
        h.keys("g");
        let Msg::GitStatus(mut roots) = answer(&h, &[]) else {
            unreachable!()
        };
        for (_, status) in &mut roots[..2] {
            let Ok(Some(s)) = status else { unreachable!() };
            s.files.retain(|f| f.unstaged != Some(GitChange::Untracked));
            s.files.retain(|f| f.unstaged != Some(GitChange::Deleted));
        }
        h.send(Msg::GitStatus(roots));
        assert_eq!(on(&h), "      M  src/x.rs");
        assert!(h.text().contains("  ▾ Changes    1\n      M  src/x.rs\n"));
    }

    #[test]
    fn a_heading_acts_on_every_file_under_it() {
        let mut h = view();
        let first = answer(&h, &[]);
        h.send(first);
        h.keys("M-< C-n C-n C-n");
        assert_eq!(on(&h), "  ▾ Changes    2");
        h.keys("s");
        let (list, _, steps) = update(&mut h).expect("asked");
        assert_eq!(steps, [(GitStep::Stage, paths(&["a/src/x.rs", "a/notes"]))]);
        fail(&mut h, list);
        h.keys("M-< s");
        let (_, _, steps) = update(&mut h).expect("the root's changes");
        assert_eq!(steps, [(GitStep::Stage, paths(&["a/src/x.rs", "a/notes"]))]);
        fail(&mut h, list);
        h.keys("M-< u");
        let (_, _, steps) = update(&mut h).expect("the root's staged files");
        assert_eq!(steps, [(GitStep::Unstage, paths(&["a/src/x.rs"]))]);
    }

    #[test]
    fn a_staged_rename_unstages_both_paths() {
        let renamed = GitFile {
            old_path: Some("a/old.rs".into()),
            ..file("a/new.rs", Some(GitChange::Renamed), None)
        };
        let mut h = view();
        let first = answer(&h, &[renamed]);
        h.send(first);
        h.keys("M-< C-n C-n C-n");
        assert_eq!(on(&h), "      R  new.rs");
        h.keys("u");
        let (_, _, steps) = update(&mut h).expect("asked");
        assert_eq!(
            steps,
            [(GitStep::Unstage, paths(&["a/new.rs", "a/old.rs"]))]
        );
    }

    #[test]
    fn discarding_asks_naming_what_is_lost() {
        let mut h = view();
        let first = answer(&h, &[]);
        h.send(first);
        h.keys("M-< C-n C-n C-n C-n");
        assert_eq!(on(&h), "      M  src/x.rs");
        h.keys("k");
        let prompt = h.core.minibuffer().map(|m| m.prompt.clone());
        assert_eq!(prompt.as_deref(), Some("Discard changes to src/x.rs? "));
        h.keys("RET");
        assert_eq!(update(&mut h), None, "No is the default");
        h.keys("C-n k");
        let prompt = h.core.minibuffer().map(|m| m.prompt.clone());
        assert_eq!(prompt.as_deref(), Some("Delete untracked notes? "));
        h.keys("yes RET");
        let (_, _, steps) = update(&mut h).expect("after yes");
        let none = Vec::new();
        assert_eq!(
            steps,
            [
                (GitStep::Restore, none.clone()),
                (GitStep::Delete, paths(&["a/notes"])),
                (GitStep::Unstage, none),
            ]
        );
    }

    #[test]
    fn discarding_a_staged_file_unstages_it_and_a_root_keeps_its_staged_text() {
        let mut h = view();
        let first = answer(&h, &[]);
        h.send(first);
        h.keys("M-< C-n C-n");
        assert_eq!(on(&h), "      M  src/x.rs");
        h.keys("k");
        assert!(h.core.minibuffer().is_none(), "nothing is lost");
        let (list, _, steps) = update(&mut h).expect("asked");
        let none = Vec::new();
        assert_eq!(
            steps,
            [
                (GitStep::Restore, none.clone()),
                (GitStep::Delete, none),
                (GitStep::Unstage, paths(&["a/src/x.rs"])),
            ]
        );
        fail(&mut h, list);
        h.keys("M-< k");
        let prompt = h.core.minibuffer().map(|m| m.prompt.clone());
        assert_eq!(
            prompt.as_deref(),
            Some("Discard changes to src/x.rs and delete untracked notes? ")
        );
        h.keys("yes RET");
        let (_, _, steps) = update(&mut h).expect("after yes");
        // Restored before unstaged, so the staged text stays in the working tree.
        assert_eq!(
            steps,
            [
                (GitStep::Restore, paths(&["a/src/x.rs"])),
                (GitStep::Delete, paths(&["a/notes"])),
                (GitStep::Unstage, paths(&["a/src/x.rs"])),
            ]
        );
    }

    #[test]
    fn unsaved_edits_refuse_staging_and_discarding() {
        let mut h = Harness::new(&[("a/src/x.rs", "1")]);
        h.open("a/src/x.rs");
        h.keys("x");
        let project = &mut h.core.projects[0].project;
        project.primary.path = h.dir.join("a");
        project.roots = Vec::new();
        h.call("diff-status", Args::default());
        let Msg::GitStatus(roots) = answer(&h, &[]) else {
            unreachable!()
        };
        h.send(Msg::GitStatus(roots[..1].to_vec()));
        h.keys("M-< C-n C-n C-n C-n");
        assert_eq!(on(&h), "      M  src/x.rs");
        h.keys("s");
        assert_eq!(h.core.message(), Some("Save src/x.rs first"));
        h.keys("k");
        assert_eq!(h.core.message(), Some("src/x.rs has unsaved edits"));
        assert!(h.core.minibuffer().is_none());
        assert_eq!(update(&mut h), None);
        h.keys("M-< C-n C-n u");
        assert!(update(&mut h).is_some(), "unstaging leaves the disk alone");
    }

    #[test]
    fn a_failure_echoes_git_and_leaves_the_list() {
        let mut h = view();
        let first = answer(&h, &[]);
        h.send(first);
        h.keys("C-n C-n s");
        let (list, _, _) = update(&mut h).expect("asked");
        let (text, asked) = (h.text(), statuses_asked(&h));
        let result = Err("fatal: Unable to create index.lock".into());
        h.send(Msg::GitUpdated { list, result });
        assert_eq!(h.core.message(), Some("fatal: Unable to create index.lock"));
        assert_eq!((h.text(), statuses_asked(&h)), (text, asked));
    }

    #[test]
    fn the_list_hints_stage_unstage_and_discard() {
        let mut h = view();
        let first = answer(&h, &[]);
        h.send(first);
        let hints = h.frame(1440., 900.).key_hints;
        assert!(
            hints.contains(&("s/u".into(), "stage/unstage")),
            "{hints:?}"
        );
        assert!(hints.contains(&("k".into(), "discard")), "{hints:?}");
    }

    #[test]
    fn a_refresh_asks_the_shown_diff_again() {
        let mut h = inline(false, true, Some("a\nB\n"));
        answer_diff(&mut h, "a\nb\n", "a\nB\n");
        h.keys("g");
        let a = h.dir.join("a");
        let status = GitStatus {
            repository: a.clone(),
            branch: "main".into(),
            files: vec![file("x.rs", None, Some(GitChange::Modified))],
        };
        h.send(Msg::GitStatus(vec![(a, Ok(Some(status)))]));
        let asked = (h.effects.iter()).any(|e| matches!(e, Effect::File(FileOp::GitDiff { .. })));
        assert!(asked, "asked again");
    }

    /// Answers a refresh with `x.rs` of `a` in these states.
    fn status_x(h: &mut Harness, staged: Option<GitChange>, unstaged: Option<GitChange>) {
        let a = h.dir.join("a");
        let status = GitStatus {
            repository: a.clone(),
            branch: "main".into(),
            files: vec![file("x.rs", staged, unstaged)],
        };
        h.send(Msg::GitStatus(vec![(a, Ok(Some(status)))]));
    }

    /// The newest `GitUpdate`'s one step, a patch, taken.
    fn patched(h: &mut Harness) -> String {
        let (_, _, steps) = update(h).expect("asked");
        match &steps[..] {
            [(GitStep::Apply(patch), paths)] if paths.is_empty() => patch.clone(),
            steps => panic!("{steps:?}"),
        }
    }

    fn x_patch(base: &str, edited: &str) -> String {
        crate::buffer::patch(Path::new("x.rs"), base, edited)
    }

    const TEN: &str = "a\nb\nc\nd\ne\nf\ng\nh\ni\nj\n";

    #[test]
    fn a_second_stage_waits_for_git() {
        let mut h = view();
        let first = answer(&h, &[]);
        h.send(first);
        h.keys("C-n C-n s");
        assert!(update(&mut h).is_some());
        h.keys("C-n s");
        assert_eq!(h.core.message(), Some("Git is still updating"));
        assert_eq!(update(&mut h), None);
    }

    #[test]
    fn a_change_waits_for_the_refreshed_list() {
        let new = "A\nb\nc\nd\nE\nf\ng\nh\ni\nJ\n";
        let mut h = inline(false, true, Some(new));
        answer_diff(&mut h, TEN, new);
        h.keys("C-x o C-c s");
        patched(&mut h);
        let list = h.core.diff_list_here().unwrap();
        h.send(Msg::GitUpdated {
            list,
            result: Ok(()),
        });
        h.keys("C-c n C-c s");
        assert_eq!(h.core.message(), Some("Git is still updating"));
        assert_eq!(update(&mut h), None, "no patch built");
    }

    #[test]
    fn the_refreshed_list_ends_the_wait() {
        let mut h = view();
        let first = answer(&h, &[]);
        h.send(first);
        h.keys("C-n C-n s");
        let (list, ..) = update(&mut h).expect("asked");
        h.send(Msg::GitUpdated {
            list,
            result: Ok(()),
        });
        let refreshed = answer(&h, &[]);
        h.send(refreshed);
        h.keys("C-n s");
        assert!(update(&mut h).is_some());
    }

    #[test]
    fn a_failure_ends_the_wait() {
        let mut h = view();
        let first = answer(&h, &[]);
        h.send(first);
        h.keys("C-n C-n s");
        let (list, ..) = update(&mut h).expect("asked");
        h.send(Msg::GitUpdated {
            list,
            result: Err("index.lock exists".into()),
        });
        assert_eq!(h.core.message(), Some("index.lock exists"));
        h.keys("s");
        assert!(update(&mut h).is_some());
    }

    #[test]
    fn a_refresh_alone_doesnt_wait() {
        let mut h = view();
        let first = answer(&h, &[]);
        h.send(first);
        h.keys("C-n C-n g s");
        assert!(update(&mut h).is_some());
    }

    #[test]
    fn staging_one_change_of_several_keeps_point_on_it() {
        let new = "A\nb\nc\nd\nE\nf\ng\nh\ni\nJ\n";
        let mut h = inline(false, true, Some(new));
        answer_diff(&mut h, TEN, new);
        h.keys("C-x o C-c n");
        assert_eq!(point_line(&h), 4);
        h.keys("C-c s");
        let staged = "a\nb\nc\nd\nE\nf\ng\nh\ni\nj\n";
        assert_eq!(patched(&mut h), x_patch(TEN, staged));
        let list = h.core.diff_list_here().unwrap();
        h.send(Msg::GitUpdated {
            list,
            result: Ok(()),
        });
        status_x(&mut h, Some(GitChange::Modified), Some(GitChange::Modified));
        answer_diff(&mut h, staged, new);
        assert_eq!(point_line(&h), 4, "not back on the first change");
        assert!(mode_line(&mut h).contains("2 changes"));
    }

    #[test]
    fn a_region_stages_its_added_lines_and_its_changes_removed_ones() {
        let new = "a\nX\nY\nZ\nc\n";
        let mut h = inline(false, true, Some(new));
        answer_diff(&mut h, "a\nb\nc\n", new);
        h.keys("C-x o C-n C-SPC C-n C-c s");
        assert_eq!(patched(&mut h), x_patch("a\nb\nc\n", "a\nY\nc\n"));
        assert!(!h.core.windows.window.active(), "the region is done with");
    }

    #[test]
    fn unstaging_a_change_puts_head_back_in_the_index() {
        let index = "a\nB\nc\nD\n";
        let mut h = inline(true, false, None);
        answer_diff(&mut h, "a\nb\nc\n", index);
        h.keys("C-x o C-c s");
        assert_eq!(h.core.message(), Some("Already staged"));
        h.keys("C-c n C-c u");
        assert_eq!(patched(&mut h), x_patch(index, "a\nB\nc\n"));
        let list = h.core.diff_list_here().unwrap();
        fail(&mut h, list);
        h.keys("C-c k");
        assert_eq!(
            patched(&mut h),
            x_patch(index, "a\nB\nc\n"),
            "discard unstages"
        );
    }

    #[test]
    fn discarding_a_change_edits_the_buffer_and_undo_brings_it_back() {
        let new = "a\nB\nc\n";
        let mut h = inline(false, true, Some(new));
        answer_diff(&mut h, "a\nb\nc\n", new);
        h.keys("C-x o C-c u");
        assert_eq!(h.core.message(), Some("Not staged"));
        h.keys("C-c k");
        assert_eq!(update(&mut h), None, "git isn't run");
        assert_eq!(right(&h).text().to_string(), "a\nb\nc\n");
        assert!(right(&h).modified(), "left unsaved");
        h.keys("C-/");
        assert_eq!(right(&h).text().to_string(), new);
    }

    #[test]
    fn discarding_a_region_keeps_the_other_added_lines() {
        // The old last line has no newline, so one is added when kept lines follow it.
        let new = "a\nB\nC\n";
        let mut h = inline(false, true, Some(new));
        answer_diff(&mut h, "a\nb", new);
        h.keys("C-x o C-n C-SPC C-n C-c k");
        assert_eq!(right(&h).text().to_string(), "a\nb\nB\n");
    }

    #[test]
    fn staging_refuses_unsaved_edits_and_point_off_a_change() {
        let mut h = inline(false, true, Some(TEN));
        answer_diff(&mut h, "a\n", TEN);
        h.keys("C-x o M-> C-c s");
        assert_eq!(h.core.message(), Some("No change here"));
        h.keys("z C-c s");
        assert_eq!(h.core.message(), Some("Save x.rs first"));
        h.keys("C-c k");
        assert_eq!(h.core.message(), Some("The diff is being worked out"));
        assert_eq!(update(&mut h), None);
    }

    #[test]
    fn an_untracked_file_stages_whole() {
        let mut h = inline(false, true, Some("new\n"));
        h.keys("g");
        status_x(&mut h, None, Some(GitChange::Untracked));
        answer_diff(&mut h, "", "new\n");
        h.keys("C-x o C-c s");
        let (_, _, steps) = update(&mut h).expect("asked");
        assert_eq!(steps, [(GitStep::Stage, paths(&["x.rs"]))]);
    }

    #[test]
    fn the_change_keys_in_the_list_act_on_files() {
        let mut h = inline(false, true, Some("a\nB\n"));
        answer_diff(&mut h, "a\nb\n", "a\nB\n");
        h.keys("C-c s");
        let (_, _, steps) = update(&mut h).expect("asked");
        assert_eq!(steps, [(GitStep::Stage, paths(&["x.rs"]))]);
    }

    #[test]
    fn a_staged_rename_unstages_whole_with_its_old_path() {
        let mut h = inline(true, false, None);
        h.keys("g");
        let a = h.dir.join("a");
        let mut renamed = file("x.rs", Some(GitChange::Renamed), None);
        renamed.old_path = Some("old.rs".into());
        let status = GitStatus {
            repository: a.clone(),
            branch: "main".into(),
            files: vec![renamed],
        };
        h.send(Msg::GitStatus(vec![(a, Ok(Some(status)))]));
        answer_diff(&mut h, "", "a\n");
        h.keys("C-x o C-c u");
        let (_, _, steps) = update(&mut h).expect("asked");
        assert_eq!(steps, [(GitStep::Unstage, paths(&["x.rs", "old.rs"]))]);
    }

    #[test]
    fn a_staged_diff_wraps_as_its_file_does() {
        let mut h = inline_file("x.md", true, false, None);
        answer_diff(&mut h, "a\n", "b\n");
        assert_eq!(right(&h).name, "x.md (index)");
        assert_eq!(right(&h).wrap(40), Some((39, true)), "at words, as prose");
    }

    /// A harness whose project has the git root `a` and `b` outside git, asked for revisions.
    fn revisions(more: &[&str]) -> Harness {
        let mut h = Harness::new(&[]);
        let project = &mut h.core.projects[0].project;
        project.primary.path = h.dir.join("a");
        project.roots = ["b"]
            .iter()
            .chain(more)
            .map(|r| Root {
                path: h.dir.join(r),
            })
            .collect();
        h.call("diff-revisions", Args::default());
        let Some(Effect::File(FileOp::GitRefs { request, roots })) = take(&mut h, |e| {
            matches!(e, Effect::File(FileOp::GitRefs { .. }))
        }) else {
            panic!("refs asked")
        };
        let refs = ["main", "topic", "v1"].map(String::from).to_vec();
        let roots = (roots.into_iter())
            .map(|root| {
                let git = root
                    .ends_with("b")
                    .not()
                    .then(|| (root.parent().unwrap().to_path_buf(), refs.clone()));
                (root, Ok(git))
            })
            .collect();
        h.send(Msg::GitRefs { request, roots });
        h
    }

    fn take(h: &mut Harness, f: impl Fn(&Effect) -> bool) -> Option<Effect> {
        let i = h.effects.iter().rposition(f)?;
        Some(h.effects.remove(i))
    }

    fn prompt(h: &mut Harness) -> Option<String> {
        h.core.minibuffer().map(|m| m.prompt.clone())
    }

    /// Takes the newest file listing asked: its request, root and revisions.
    fn listing(h: &mut Harness) -> Option<(u64, PathBuf, String, Option<String>)> {
        match take(h, |e| {
            matches!(e, Effect::File(FileOp::GitDiffFiles { .. }))
        })? {
            Effect::File(FileOp::GitDiffFiles {
                request,
                root,
                old,
                new,
                ..
            }) => Some((request, root, old, new)),
            _ => unreachable!(),
        }
    }

    fn changed(path: &str, change: GitChange) -> ChangedFile {
        ChangedFile {
            path: path.into(),
            change,
            old_path: None,
        }
    }

    #[test]
    fn revisions_list_the_files_between_two_revisions_read_only() {
        use GitChange::*;
        let mut h = revisions(&[]);
        assert_eq!(
            prompt(&mut h).as_deref(),
            Some("Old revision: "),
            "one git root"
        );
        assert_eq!(
            h.core.minibuffer().unwrap().candidates,
            ["main", "topic", "v1"]
        );
        h.keys("top RET");
        assert_eq!(
            prompt(&mut h).as_deref(),
            Some("New revision (empty for working tree): ")
        );
        h.keys("v1 RET");
        let (request, root, old, new) = listing(&mut h).expect("listed");
        assert_eq!(
            (root, old, new),
            (h.dir.join("a"), "topic".into(), Some("v1".into()))
        );
        let files = [
            changed("a/mod", Modified),
            changed("a/new", Added),
            changed("a/gone", Deleted),
        ];
        h.send(Msg::GitDiffFiles {
            request,
            result: Ok(files.to_vec()),
        });
        assert_eq!(
            h.text(),
            "▾ a  topic → v1\n      M  mod\n      A  new\n      D  gone"
        );
        assert_eq!(h.core.windows.tree.leaves().len(), 2, "the view is open");
        h.keys("TAB");
        assert_eq!(h.text(), "▸ a  topic → v1");
        h.keys("TAB C-n");
        h.frame(1440., 900.);
        let sides = answer_diff(&mut h, "a\n", "b\n");
        assert_eq!(
            sides,
            (GitSide::Tree("topic".into()), GitSide::Tree("v1".into()))
        );
        assert_eq!(right(&h).name, "mod (v1)");
        assert!(right(&h).read_only);
        assert!(
            mode_line(&mut h).contains("topic → v1"),
            "{}",
            mode_line(&mut h)
        );
        let hints = h.frame(1440., 900.).key_hints;
        let has = |name| hints.iter().any(|(_, what)| *what == name);
        assert!(
            !has("discard") && !has("stage/unstage") && has("refresh"),
            "{hints:?}"
        );
        for key in ["s", "u", "k", "C-x o C-c s"] {
            h.keys(key);
            assert_eq!(
                h.core.message(),
                Some("This diff can't be changed"),
                "{key}"
            );
            assert_eq!(update(&mut h), None);
        }
        h.keys("C-c g");
        let (request, root, old, new) = listing(&mut h).expect("asked again");
        assert_eq!(
            (root, old, new),
            (h.dir.join("a"), "topic".into(), Some("v1".into()))
        );
        let moved = [changed("a/mod", Modified), changed("a/later", Added)];
        h.send(Msg::GitDiffFiles {
            request,
            result: Ok(moved.to_vec()),
        });
        h.keys("C-x o");
        assert_eq!(h.text(), "▾ a  topic → v1\n      M  mod\n      A  later");
        h.keys("g");
        let (request, ..) = listing(&mut h).expect("asked again");
        h.send(Msg::GitDiffFiles {
            request,
            result: Err("fatal: bad revision 'topic'".into()),
        });
        assert_eq!(h.text(), "  a  fatal: bad revision 'topic'");
    }

    #[test]
    fn revisions_against_the_working_tree_edit_the_files_buffer() {
        let mut h = revisions(&[]);
        h.read_reply("a/mod", Some(b"b\n"));
        h.keys("HEAD RET RET");
        let (request, _, old, new) = listing(&mut h).expect("listed");
        assert_eq!(
            (old.as_str(), new),
            ("HEAD", None),
            "empty is the working tree"
        );
        h.send(Msg::GitDiffFiles {
            request,
            result: Ok(vec![changed("a/mod", GitChange::Modified)]),
        });
        assert_eq!(h.text(), "▾ a  HEAD → working tree\n      M  mod");
        h.frame(1440., 900.);
        let sides = answer_diff(&mut h, "a\n", "b\n");
        assert_eq!(sides, (GitSide::Tree("HEAD".into()), GitSide::WorkingTree));
        assert_eq!(right(&h).path(), Some(h.path("a/mod").as_path()));
        assert!(mode_line(&mut h).contains("HEAD → working tree"));
        h.keys("C-x o x");
        assert_eq!(right(&h).text().to_string(), "xb\n");
    }

    #[test]
    fn several_git_roots_ask_for_one_and_bad_answers_open_nothing() {
        let mut h = revisions(&["c"]);
        assert_eq!(prompt(&mut h).as_deref(), Some("Root: "));
        let [a, c] = ["a", "c"].map(|r| h.dir.join(r).display().to_string());
        let roots = [a, c.clone()];
        assert_eq!(
            h.core.minibuffer().unwrap().candidates,
            roots,
            "git roots only"
        );
        h.keys(&format!("{c} RET RET"));
        assert_eq!(h.core.message(), Some("No revision"));
        assert_eq!(listing(&mut h), None);
        let mut h = revisions(&[]);
        h.keys("nope RET RET");
        let (request, ..) = listing(&mut h).expect("listed");
        let error = "fatal: bad revision 'nope'";
        h.send(Msg::GitDiffFiles {
            request,
            result: Err(error.into()),
        });
        assert_eq!(h.core.message(), Some(error));
        assert_eq!(h.core.windows.tree.leaves().len(), 1, "nothing opened");
    }

    #[test]
    fn no_git_root_says_so() {
        let mut h = Harness::new(&[]);
        let project = &mut h.core.projects[0].project;
        (project.primary.path, project.roots) = (h.dir.join("b"), Vec::new());
        h.call("diff-revisions", Args::default());
        let Some(Effect::File(FileOp::GitRefs { request, roots })) = take(&mut h, |e| {
            matches!(e, Effect::File(FileOp::GitRefs { .. }))
        }) else {
            panic!("refs asked")
        };
        let roots = roots.into_iter().map(|r| (r, Ok(None))).collect();
        h.send(Msg::GitRefs { request, roots });
        assert_eq!(h.core.message(), Some("No git root"));
    }

    #[test]
    fn status_and_revisions_replace_each_others_view() {
        let mut h = revisions(&[]);
        h.keys("main RET RET");
        let (request, ..) = listing(&mut h).expect("listed");
        h.send(Msg::GitDiffFiles {
            request,
            result: Ok(Vec::new()),
        });
        assert!(
            h.text().starts_with("  a  main → working tree"),
            "{}",
            h.text()
        );
        h.call("diff-status", Args::default());
        assert_eq!(h.text(), "Reading status...");
        assert_eq!(h.core.windows.tree.leaves().len(), 2);
        h.call("diff-revisions", Args::default());
        let Some(Effect::File(FileOp::GitRefs { request, roots })) = take(&mut h, |e| {
            matches!(e, Effect::File(FileOp::GitRefs { .. }))
        }) else {
            panic!("refs asked")
        };
        let roots = (roots.into_iter())
            .map(|r| {
                let git = r
                    .ends_with("a")
                    .then(|| (h.dir.clone(), vec!["main".to_string()]));
                (r, Ok(git))
            })
            .collect();
        h.send(Msg::GitRefs { request, roots });
        h.keys("main RET v1 RET");
        let (request, ..) = listing(&mut h).expect("listed");
        assert_eq!(
            h.text(),
            "Reading status...",
            "the status view stays until the answer"
        );
        h.send(Msg::GitDiffFiles {
            request,
            result: Ok(Vec::new()),
        });
        assert!(h.text().starts_with("  a  main → v1"), "{}", h.text());
        assert_eq!(h.core.windows.tree.leaves().len(), 2);
        h.keys("q");
        assert_eq!(
            h.core.windows.tree.leaves().len(),
            1,
            "one layout saved, not two"
        );
    }
}
