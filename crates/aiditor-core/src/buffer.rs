//! Buffers: content addressed by id, edited by char offset.

use std::collections::BTreeMap;
use std::fmt;
use std::ops::Range;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use ropey::Rope;
use serde::Deserialize;

use crate::settings::{FaceName, LineNumbers};
use crate::syntax::{self, Syntax};
use crate::terminal::{ShellCommand, Terminal};
use crate::workspace::WorkspaceId;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BufferId(u64);

impl BufferId {
    /// The id's number, unique within one process.
    pub fn number(self) -> u64 {
        self.0
    }
}

/// A position in one buffer that follows edits; ids are not reused within a buffer.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MarkerId(u64);

/// What a marker does when text is inserted exactly at it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InsertionType {
    /// Stays before the inserted text.
    Stay,
    /// Moves past the inserted text.
    Advance,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Error {
    NoSuchBuffer(BufferId),
    OutOfRange,
    ReadOnly,
    /// The edit touches text before the prompt boundary.
    TextReadOnly,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Error::NoSuchBuffer(id) => write!(f, "no buffer {}", id.0),
            Error::OutOfRange => f.write_str("position out of range"),
            Error::ReadOnly => f.write_str("Buffer is read-only"),
            Error::TextReadOnly => f.write_str("Text is read-only"),
        }
    }
}

impl std::error::Error for Error {}

impl From<Error> for String {
    fn from(err: Error) -> Self {
        err.to_string()
    }
}

impl From<ropey::Error> for Error {
    fn from(_: ropey::Error) -> Self {
        Error::OutOfRange
    }
}

/// `base`, or `base<2>`, `base<3>`, ... while `taken` says the name is in use.
pub(crate) fn unique(base: &str, taken: impl Fn(&str) -> bool) -> String {
    let mut name = base.to_string();
    for n in 2.. {
        if !taken(&name) {
            break;
        }
        name = format!("{base}<{n}>");
    }
    name
}

/// The name of the `*scratch*` of the workspace named `workspace`.
pub(crate) fn scratch_name(workspace: &str) -> String {
    format!("*scratch*<{workspace}>")
}

/// The last component of `path`, or the whole path when it has none, such as `/`.
pub(crate) fn file_name(path: &Path) -> String {
    path.file_name()
        .unwrap_or(path.as_os_str())
        .to_string_lossy()
        .into_owned()
}

/// Every buffer in the core. Ids only increase, so a stale id never finds a newer buffer.
#[derive(Default)]
pub struct Buffers {
    next: u64,
    map: BTreeMap<BufferId, Buffer>,
    /// Every buffer, most recently shown first; new buffers go last, as in Emacs's `buffer-list`.
    recent: Vec<BufferId>,
    /// The current workspace, which owns every buffer made from now on.
    pub(crate) workspace: WorkspaceId,
}

impl Buffers {
    pub fn create(&mut self, name: &str) -> BufferId {
        self.add(Buffer {
            name: name.to_string(),
            workspace: Some(self.workspace),
            content: Content::Text(Rope::new()),
            file: None,
            crlf: false,
            bom: false,
            disk: None,
            deleted: false,
            conflict: None,
            conflict_asked: false,
            head: None,
            head_asked: false,
            read_only: false,
            truncate_lines: false,
            line_numbers: None,
            unkillable: false,
            modified: false,
            tick: 0,
            markers: BTreeMap::from([(MarkerId(0), (0, InsertionType::Stay))]),
            next_marker: 1,
            props: Vec::new(),
            props_sorted: true,
            history: History::default(),
            major_mode: "fundamental-mode",
            minor_modes: Vec::new(),
            workspace_rows: Vec::new(),
            point: MarkerId(0),
            commands: Vec::new(),
            syntax: None,
            prompt: None,
            agent_mode: None,
        })
    }

    /// A new unkillable `*scratch*<workspace>`.
    pub fn create_scratch(&mut self, workspace: &str) -> BufferId {
        let id = self.create(&scratch_name(workspace));
        self.map.get_mut(&id).expect("just created").unkillable = true;
        id
    }

    /// A new read-only buffer named `name`, made unique, holding `terminal`'s grid.
    pub fn create_terminal(&mut self, name: &str, terminal: Terminal) -> BufferId {
        let id = self.create(&unique(name, |n| self.find(n).is_some()));
        let buffer = self.map.get_mut(&id).expect("just created");
        (buffer.content, buffer.read_only) = (Content::Terminal(Box::new(terminal), None), true);
        buffer.major_mode = "vterm-mode";
        id
    }

    /// Renames buffer `id` to `name`, appending `<2>`, `<3>`, ... while another buffer has it.
    pub(crate) fn rename(&mut self, id: BufferId, name: &str) {
        let taken = |n: &str| (self.map.iter()).any(|(&b, buffer)| b != id && buffer.name == n);
        let name = unique(name, taken);
        if let Some(buffer) = self.map.get_mut(&id) {
            buffer.name = name;
        }
    }

    /// A buffer visiting `path`, holding `text` already read from it.
    pub fn create_for_file(&mut self, path: &Path, text: &str) -> BufferId {
        self.create_for_prepared_file(path, Rope::from_str(text))
    }

    pub(crate) fn create_for_prepared_file(&mut self, path: &Path, text: Rope) -> BufferId {
        let id = self.create(&file_name(path));
        let buffer = self.map.get_mut(&id).expect("just created");
        buffer.syntax = Syntax::for_file(path, &text);
        buffer.content = Content::Text(text);
        buffer.file = Some(path.to_path_buf());
        id
    }

    /// Adds `buffer`, appending `<2>`, `<3>`, ... to its name while another buffer has it.
    fn add(&mut self, mut buffer: Buffer) -> BufferId {
        buffer.name = unique(&buffer.name, |name| {
            self.map.values().any(|b| b.name == name)
        });
        let id = BufferId(self.next);
        self.next += 1;
        self.map.insert(id, buffer);
        self.recent.push(id);
        id
    }

    /// Every buffer, most recently shown first.
    pub fn recent(&self) -> impl Iterator<Item = (BufferId, &Buffer)> {
        self.recent.iter().map(|&id| (id, &self.map[&id]))
    }

    /// Makes buffer `id` the most recently shown.
    pub fn raise(&mut self, id: BufferId) {
        self.recent.retain(|&b| b != id);
        self.recent.insert(0, id);
    }

    /// The buffer named `name`.
    pub fn find(&self, name: &str) -> Option<BufferId> {
        self.map
            .iter()
            .find(|(_, b)| b.name == name)
            .map(|(&id, _)| id)
    }

    /// The buffer named `name` that the current workspace owns, or `*Messages*`.
    pub fn find_here(&self, name: &str) -> Option<BufferId> {
        self.find(name).filter(|&id| self.is_here(id))
    }

    /// Buffer `id` belongs to the current workspace or to none.
    pub fn is_here(&self, id: BufferId) -> bool {
        self.map[&id].workspace.is_none_or(|w| w == self.workspace)
    }

    /// The current workspace's buffer visiting `path`.
    pub fn visiting(&self, path: &Path) -> Option<BufferId> {
        self.map
            .iter()
            .find(|(_, b)| b.workspace == Some(self.workspace) && b.file.as_deref() == Some(path))
            .map(|(&id, _)| id)
    }

    pub fn get(&self, id: BufferId) -> Option<&Buffer> {
        self.map.get(&id)
    }

    pub fn get_mut(&mut self, id: BufferId) -> Option<&mut Buffer> {
        self.map.get_mut(&id)
    }

    /// Every buffer, in any workspace.
    pub(crate) fn iter_mut(&mut self) -> impl Iterator<Item = (BufferId, &mut Buffer)> {
        self.map.iter_mut().map(|(&id, b)| (id, b))
    }

    pub fn kill(&mut self, id: BufferId) -> Result<Buffer, Error> {
        self.recent.retain(|&b| b != id);
        self.map.remove(&id).ok_or(Error::NoSuchBuffer(id))
    }
}

/// The content backend.
pub enum Content {
    Text(Rope),
    /// A terminal's grid, and copy mode's text built from its scrollback while in copy mode.
    Terminal(Box<Terminal>, Option<Rope>),
}

static EMPTY: LazyLock<Rope> = LazyLock::new(Rope::new);

pub struct Buffer {
    pub name: String,
    /// The workspace that owns it; `None` only for the global `*Messages*`.
    pub workspace: Option<WorkspaceId>,
    content: Content,
    pub file: Option<PathBuf>,
    /// The file's line breaks are CRLF, written back on save.
    pub crlf: bool,
    /// The file starts with a UTF-8 byte-order mark, written back on save.
    pub bom: bool,
    /// The hash of the file's bytes as last loaded or saved; `None` when there was no file.
    pub disk: Option<u64>,
    /// The file was found deleted on disk; a save writes it back.
    pub deleted: bool,
    /// The hash of the disk version its unsaved edits conflict with, until resolved.
    pub conflict: Option<u64>,
    /// Focusing it already asked about `conflict`; a save still asks.
    pub conflict_asked: bool,
    /// The repository of the visited file, if it is in one.
    pub head: Option<crate::Head>,
    /// `head` was asked for since it last may have changed.
    pub head_asked: bool,
    pub read_only: bool,
    /// Long lines are cut at the window edge instead of wrapping, as Emacs's `truncate-lines`.
    pub truncate_lines: bool,
    /// `display-line-numbers-mode`'s choice for this buffer; `None` follows the setting.
    pub line_numbers: Option<bool>,
    /// `kill-buffer` refuses it; killing its workspace still kills it.
    pub unkillable: bool,
    modified: bool,
    /// Counts text changes, like Emacs's `buffer-modified-tick`.
    tick: u64,
    // shortcut: every edit walks all markers, two per property, so a long session transcript holds thousands; index them when typing in one lags.
    markers: BTreeMap<MarkerId, (usize, InsertionType)>,
    next_marker: u64,
    /// Text properties as (start, end) marker pairs, in order while sorted, else oldest first.
    props: Vec<(MarkerId, MarkerId, Prop)>,
    /// Starts and ends each ascend from one property to the next, which edits keep, so
    /// `properties_near` can bisect.
    props_sorted: bool,
    history: History,
    /// Names the major mode's keymap in `Core`.
    pub major_mode: &'static str,
    /// Names the minor modes' keymaps in `Core`, highest priority first.
    pub minor_modes: Vec<&'static str>,
    pub(crate) workspace_rows: Vec<(usize, WorkspaceId)>,
    /// The point of the last window that stopped showing this buffer, where the next one starts.
    pub point: MarkerId,
    /// In copy mode, each command's marks in the text, sorted by start.
    pub(crate) commands: Vec<ShellCommand>,
    /// The parse of a file in a language aiditor highlights.
    syntax: Option<Syntax>,
    /// A `Stay` marker at a session buffer's prompt area: the text before it is read-only.
    pub(crate) prompt: Option<MarkerId>,
    /// A session buffer's agent's current mode, for the mode line.
    pub(crate) agent_mode: Option<String>,
}

/// One recorded edit: where, and the text inserted or deleted there.
#[derive(Clone, Debug)]
enum Change {
    Insert(usize, String),
    Delete(usize, String),
}

/// The edits of one command, and point before and after it.
struct Group {
    changes: Vec<Change>,
    before: usize,
    after: usize,
}

// shortcut: history grows without bound; cap it when long sessions show memory growth.
#[derive(Default)]
struct History {
    undo: Vec<Group>,
    redo: Vec<Group>,
    /// The last undo group still takes edits.
    open: bool,
    /// Point at the last boundary, for the next group.
    before: Option<usize>,
}

/// Colors as 0xRRGGBB, `None` keeping whatever is under it, and font attributes, added to those
/// under it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Face {
    pub fg: Option<u32>,
    pub bg: Option<u32>,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
}

impl Face {
    /// This face drawn over `under`.
    pub fn over(self, under: Face) -> Face {
        Face {
            fg: self.fg.or(under.fg),
            bg: self.bg.or(under.bg),
            bold: self.bold || under.bold,
            italic: self.italic || under.italic,
            underline: self.underline || under.underline,
        }
    }
}

/// One layer of a stored face, named so a theme change needs no rewrite.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FaceRef {
    Face(FaceName),
    /// A syntax capture's face.
    Syntax(&'static str),
    /// An ANSI colour's index, as text colour or background.
    Fg(u8),
    Bg(u8),
    Bold,
    Italic,
    Underline,
}

/// Something drawn across a whole line, in a face's `bg` or, for a rule, its `fg`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Decoration {
    Background(FaceName),
    /// A rule beside the line, like the one beside tool calls.
    Rule(FaceName),
}

/// A text property on a char range.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Prop {
    Face(Face),
    /// Layers resolved against the theme at each redisplay, later ones on top.
    Named(Vec<FaceRef>),
    /// Hidden text, shown as this placeholder (empty for none).
    Invisible(String),
    /// Applies to every line the range touches.
    Line(Decoration),
}

impl Buffer {
    /// The chars a visual row holds in a window `cols` wide and whether rows break at words, or
    /// `None` when long lines are cut instead; the last column is left for the `\` marker.
    pub(crate) fn wrap(&self, cols: usize) -> Option<(usize, bool)> {
        let prose = (self.file.as_deref().and_then(Path::extension))
            .is_some_and(|e| ["md", "markdown", "txt"].iter().any(|p| e == *p));
        let word = prose || self.major_mode == crate::agents::MODE;
        // The minibuffer is one line, so it always truncates.
        let truncate = self.truncate_lines || self.major_mode == "minibuffer-mode";
        (!truncate && cols >= 2).then(|| (cols - 1, word))
    }

    /// Whether it can show line numbers: a file, `*scratch*` or `*Messages*`, the only unkillable
    /// buffers; the design draws no gutter in sessions, terminals or grep.
    pub(crate) fn numbered(&self) -> bool {
        self.file.is_some() || self.unkillable
    }

    /// How this buffer's lines are numbered under the `line_numbers` setting.
    pub(crate) fn line_numbers(&self, setting: LineNumbers) -> LineNumbers {
        match (self.numbered(), self.line_numbers, setting) {
            (false, _, _) | (_, Some(false), _) => LineNumbers::Off,
            (_, Some(true), LineNumbers::Off) => LineNumbers::Absolute,
            _ => setting,
        }
    }

    pub fn text(&self) -> &Rope {
        match &self.content {
            Content::Text(rope) => rope,
            Content::Terminal(_, copy) => copy.as_ref().unwrap_or(&EMPTY),
        }
    }

    /// Whether this terminal buffer is in copy mode.
    pub fn copy_mode(&self) -> bool {
        matches!(self.content, Content::Terminal(_, Some(_)))
    }

    /// Sets a terminal buffer's copy-mode text, `None` to leave copy mode, past read-only and
    /// without undo, dropping the old text's properties; markers past its end move to it.
    pub(crate) fn set_copy_text(&mut self, text: Option<Rope>) {
        let Content::Terminal(_, copy) = &mut self.content else {
            return;
        };
        *copy = text;
        self.commands.clear();
        self.props_sorted = true;
        for (start, end, _) in std::mem::take(&mut self.props) {
            self.markers.remove(&start);
            self.markers.remove(&end);
        }
        self.minor_modes.retain(|&m| m != "vterm-copy-mode");
        if self.copy_mode() {
            self.minor_modes.push("vterm-copy-mode");
        }
        let len = self.len_chars();
        for (pos, _) in self.markers.values_mut() {
            *pos = (*pos).min(len);
        }
        self.tick += 1;
    }

    pub fn terminal(&self) -> Option<&Terminal> {
        match &self.content {
            Content::Terminal(terminal, _) => Some(terminal),
            Content::Text(_) => None,
        }
    }

    pub(crate) fn terminal_mut(&mut self) -> Option<&mut Terminal> {
        match &mut self.content {
            Content::Terminal(terminal, _) => Some(terminal),
            Content::Text(_) => None,
        }
    }

    pub fn insert(&mut self, char_idx: usize, text: &str) -> Result<(), Error> {
        self.raw_insert(char_idx, text)?;
        self.record(Change::Insert(char_idx, text.to_string()));
        Ok(())
    }

    pub fn delete(&mut self, range: Range<usize>) -> Result<(), Error> {
        let deleted = self.text().get_slice(range.clone()).map(String::from);
        self.raw_delete(range.clone())?;
        self.record(Change::Delete(range.start, deleted.unwrap_or_default()));
        Ok(())
    }

    /// Appends `text` at the end, past read-only and without undo, as Emacs logs to `*Messages*`.
    /// The tick stays, so a command's own echo does not count as its edit and deactivate the mark.
    pub fn append(&mut self, text: &str) {
        let (read_only, tick) = (std::mem::take(&mut self.read_only), self.tick);
        let end = self.len_chars();
        self.raw_insert(end, text).expect("the end is in range");
        (self.read_only, self.tick) = (read_only, tick);
    }

    /// Inserts `text` at `at`, before the prompt boundary, past read-only and without undo,
    /// moving the undo history's positions from `at` on along with the text.
    pub(crate) fn insert_output(&mut self, at: usize, text: &str) {
        self.replace_output(at..at, text);
    }

    /// Replaces `range` with `text` as `insert_output` inserts, the undo history's positions
    /// from the range's end on moving by the change in length.
    pub(crate) fn replace_output(&mut self, range: Range<usize>, text: &str) {
        let (read_only, prompt) = (std::mem::take(&mut self.read_only), self.prompt.take());
        (self.raw_delete(range.clone()))
            .and_then(|()| self.raw_insert(range.start, text))
            .expect("output goes inside the text");
        (self.read_only, self.prompt) = (read_only, prompt);
        let (at, n) = (range.end, text.chars().count());
        let shift = |pos: &mut usize| {
            if *pos >= at {
                *pos = *pos - range.len() + n;
            }
        };
        let h = &mut self.history;
        h.before.as_mut().map(shift);
        for group in h.undo.iter_mut().chain(&mut h.redo) {
            shift(&mut group.before);
            shift(&mut group.after);
            for change in &mut group.changes {
                match change {
                    Change::Insert(pos, _) | Change::Delete(pos, _) => shift(pos),
                }
            }
        }
    }

    /// Forgets every undo and redo group.
    pub(crate) fn clear_undo(&mut self) {
        self.history = History::default();
    }

    /// Where the prompt boundary is, 0 without one.
    pub(crate) fn prompt_start(&self) -> usize {
        self.prompt.and_then(|m| self.marker(m)).unwrap_or(0)
    }

    /// Ends the open undo group with point at `point`; the next edit starts a group with point there.
    pub fn undo_boundary(&mut self, point: usize) {
        let h = &mut self.history;
        if std::mem::take(&mut h.open) {
            h.undo.last_mut().expect("open group exists").after = point;
        }
        h.before = Some(point);
    }

    /// Reverts the last undo group; returns point before it, or `None` with nothing to undo.
    pub fn undo(&mut self) -> Option<usize> {
        self.history.open = false;
        let group = self.history.undo.pop()?;
        for change in group.changes.iter().rev() {
            self.apply(change, true);
        }
        let point = group.before;
        self.history.redo.push(group);
        Some(point)
    }

    /// Re-applies the last undone group; returns point after it, or `None` with nothing to redo.
    pub fn redo(&mut self) -> Option<usize> {
        let group = self.history.redo.pop()?;
        for change in &group.changes {
            self.apply(change, false);
        }
        let point = group.after;
        self.history.undo.push(group);
        Some(point)
    }

    /// Applies `change`, or its inverse, without recording it.
    fn apply(&mut self, change: &Change, inverse: bool) {
        let result = match (change, inverse) {
            (Change::Insert(at, text), false) | (Change::Delete(at, text), true) => {
                self.raw_insert(*at, text)
            }
            (Change::Insert(at, text), true) | (Change::Delete(at, text), false) => {
                self.raw_delete(*at..*at + text.chars().count())
            }
        };
        result.expect("history replays onto the text it was recorded on");
    }

    fn record(&mut self, change: Change) {
        let (at, text) = match &change {
            Change::Insert(at, text) | Change::Delete(at, text) => (*at, text),
        };
        if text.is_empty() {
            return;
        }
        let h = &mut self.history;
        h.redo.clear();
        if !h.open {
            h.open = true;
            h.undo.push(Group {
                changes: Vec::new(),
                before: h.before.take().unwrap_or(at),
                after: at,
            });
        }
        let group = h.undo.last_mut().expect("open group exists");
        // Until a boundary says where point went, assume just after the edit.
        group.after = match change {
            Change::Insert(..) => at + text.chars().count(),
            Change::Delete(..) => at,
        };
        group.changes.push(change);
    }

    fn raw_insert(&mut self, char_idx: usize, text: &str) -> Result<(), Error> {
        if self.read_only && !text.is_empty() {
            return Err(Error::ReadOnly);
        }
        if char_idx < self.prompt_start() && !text.is_empty() {
            return Err(Error::TextReadOnly);
        }
        let start = (self.syntax.is_some() && char_idx <= self.len_chars())
            .then(|| syntax::position(self.text(), char_idx));
        match &mut self.content {
            Content::Text(rope) => rope.try_insert(char_idx, text)?,
            // Not even the paths past read-only edit a grid.
            Content::Terminal(..) if !text.is_empty() => return Err(Error::ReadOnly),
            Content::Terminal(..) => {}
        }
        let n = text.chars().count();
        if let Some(start) = start {
            let new_end = syntax::position(self.text(), char_idx + n);
            if let Some(syntax) = &mut self.syntax {
                syntax.edit(start, start, new_end);
            }
        }
        for (pos, kind) in self.markers.values_mut() {
            if *pos > char_idx || (*pos == char_idx && *kind == InsertionType::Advance) {
                *pos += n;
            }
        }
        self.modified |= !text.is_empty();
        self.tick += u64::from(!text.is_empty());
        Ok(())
    }

    fn raw_delete(&mut self, range: Range<usize>) -> Result<(), Error> {
        let changed = !range.is_empty();
        if self.read_only && changed {
            return Err(Error::ReadOnly);
        }
        if range.start < self.prompt_start() && changed {
            return Err(Error::TextReadOnly);
        }
        let ends =
            (self.syntax.is_some() && range.start <= range.end && range.end <= self.len_chars())
                .then(|| {
                    (
                        syntax::position(self.text(), range.start),
                        syntax::position(self.text(), range.end),
                    )
                });
        match &mut self.content {
            Content::Text(rope) => rope.try_remove(range.clone())?,
            Content::Terminal(..) if changed => return Err(Error::ReadOnly),
            Content::Terminal(..) => {}
        }
        if let (Some(syntax), Some((start, old_end))) = (&mut self.syntax, ends) {
            syntax.edit(start, old_end, start);
        }
        for (pos, _) in self.markers.values_mut() {
            if *pos > range.start {
                *pos = range.start.max(pos.saturating_sub(range.len()));
            }
        }
        self.modified |= changed;
        self.tick += u64::from(changed);
        Ok(())
    }

    pub fn add_marker(&mut self, char_idx: usize, kind: InsertionType) -> Result<MarkerId, Error> {
        if char_idx > self.len_chars() {
            return Err(Error::OutOfRange);
        }
        let id = MarkerId(self.next_marker);
        self.next_marker += 1;
        self.markers.insert(id, (char_idx, kind));
        Ok(id)
    }

    /// The marker's offset, or `None` once it is removed.
    pub fn marker(&self, id: MarkerId) -> Option<usize> {
        self.markers.get(&id).map(|&(pos, _)| pos)
    }

    /// Moves a live marker to `char_idx`; a removed one is not brought back.
    pub fn set_marker(&mut self, id: MarkerId, char_idx: usize) -> Result<(), Error> {
        if char_idx > self.len_chars() {
            return Err(Error::OutOfRange);
        }
        if let Some((pos, _)) = self.markers.get_mut(&id) {
            *pos = char_idx;
        }
        Ok(())
    }

    /// Puts `prop` on `range`; text inserted inside it extends it, at its edges does not.
    pub fn add_property(&mut self, range: Range<usize>, prop: Prop) -> Result<(), Error> {
        if range.start > range.end || range.end > self.len_chars() {
            return Err(Error::OutOfRange);
        }
        let at = |id| {
            self.marker(id)
                .expect("a property's markers live as long as it")
        };
        let mut i = self.props.len();
        if self.props_sorted {
            // Its sorted place, which keeps the order if the ends around it still ascend.
            i = (self.props)
                .partition_point(|&(s, e, _)| (at(s), at(e)) <= (range.start, range.end));
            let before = i.checked_sub(1).map(|j| at(self.props[j].1));
            let after = self.props.get(i).map(|p| at(p.1));
            if before > Some(range.end) || after.is_some_and(|e| e < range.end) {
                (self.props_sorted, i) = (false, self.props.len());
            }
        }
        let start = self.add_marker(range.start, InsertionType::Advance)?;
        let end = self.add_marker(range.end, InsertionType::Stay)?;
        self.props.insert(i, (start, end, prop));
        Ok(())
    }

    /// Removes every property whose range lies within `range`.
    pub fn remove_properties(&mut self, range: Range<usize>) {
        let markers = &mut self.markers;
        self.props.retain(|(start, end, _)| {
            let at = |id| markers.get(id).map(|&(pos, _)| pos);
            let inside = at(start) >= Some(range.start) && at(end) <= Some(range.end);
            if inside {
                markers.remove(start);
                markers.remove(end);
            }
            !inside
        });
    }

    /// The non-empty property ranges touching `range`, from at its start to at its end, in
    /// stored order.
    pub fn properties_near(&self, range: Range<usize>) -> Vec<(Range<usize>, &Prop)> {
        let touches = |r: &Range<usize>| r.end >= range.start && r.start <= range.end;
        if !self.props_sorted {
            return self.properties().filter(|(r, _)| touches(r)).collect();
        }
        let at = |id| {
            self.marker(id)
                .expect("a property's markers live as long as it")
        };
        let from = (self.props).partition_point(|&(_, end, _)| at(end) < range.start);
        let to = (self.props).partition_point(|&(start, ..)| at(start) <= range.end);
        (self.props[from..to.max(from)].iter())
            .map(|(s, e, prop)| (at(*s)..at(*e), prop))
            .filter(|(r, _)| !r.is_empty())
            .collect()
    }

    /// `line`, or when it starts inside invisible text, the line after that fold (`down`) or the
    /// fold's first line, as Emacs's `line-move-ignore-invisible`.
    pub(crate) fn unfolded_line(&self, line: usize, down: bool) -> usize {
        if !self.has_folds() {
            return line;
        }
        let (text, pos) = (self.text(), self.text().line_to_char(line));
        let fold = (self.properties_near(pos..pos).into_iter())
            .find(|(r, p)| matches!(p, Prop::Invisible(_)) && r.start < pos && pos < r.end);
        match fold {
            Some((r, _)) if down => text.char_to_line(r.end) + 1,
            Some((r, _)) => text.char_to_line(r.start),
            None => line,
        }
    }

    /// Whether any property hides text.
    pub(crate) fn has_folds(&self) -> bool {
        (self.props.iter()).any(|(.., prop)| matches!(prop, Prop::Invisible(_)))
    }

    /// The non-empty property ranges, in stored order.
    pub fn properties(&self) -> impl Iterator<Item = (Range<usize>, &Prop)> {
        self.props.iter().filter_map(|(s, e, prop)| {
            let range = self.marker(*s)?..self.marker(*e)?;
            (!range.is_empty()).then_some((range, prop))
        })
    }

    /// Brings the syntax tree up to date with the text's edits.
    pub(crate) fn reparse(&mut self) {
        if let (Some(syntax), Content::Text(rope)) = (&mut self.syntax, &self.content) {
            syntax.reparse(rope);
        }
    }

    /// The name of the language its file is in, for the mode line.
    pub(crate) fn language(&self) -> &'static str {
        self.syntax.as_ref().map_or("Fundamental", |s| s.name)
    }

    /// Whether it has a syntax tree to colour its text with.
    pub(crate) fn highlighted(&self) -> bool {
        self.syntax.is_some()
    }

    /// The syntax faces `palette` gives the chars of sorted, disjoint `ranges`, as sorted,
    /// disjoint char ranges.
    pub(crate) fn highlights(
        &self,
        ranges: &[Range<usize>],
        palette: &BTreeMap<String, Face>,
    ) -> Vec<(Range<usize>, Face)> {
        (self.syntax.as_ref()).map_or(Vec::new(), |s| s.highlights(self.text(), ranges, palette))
    }

    pub fn remove_marker(&mut self, id: MarkerId) {
        self.markers.remove(&id);
    }

    pub fn len_chars(&self) -> usize {
        self.text().len_chars()
    }

    pub fn len_lines(&self) -> usize {
        self.text().len_lines()
    }

    /// Zero-based (line, column), the column in chars from the line's start.
    pub fn line_col(&self, char_idx: usize) -> Result<(usize, usize), Error> {
        let rope = self.text();
        let line = rope.try_char_to_line(char_idx)?;
        Ok((line, char_idx - rope.line_to_char(line)))
    }

    /// The offset of `line`, `col`; a column past the line's end clamps to it, before its LF.
    pub fn char_idx(&self, line: usize, col: usize) -> Result<usize, Error> {
        let rope = self.text();
        if line >= rope.len_lines() {
            return Err(Error::OutOfRange);
        }
        let start = rope.line_to_char(line);
        let slice = rope.line(line);
        let len = slice.len_chars();
        let len = len - usize::from(len > 0 && slice.char(len - 1) == '\n');
        Ok(start + col.min(len))
    }

    pub fn modified(&self) -> bool {
        self.modified
    }

    pub fn tick(&self) -> u64 {
        self.tick
    }

    pub fn set_saved(&mut self) {
        self.modified = false;
    }

    /// Replaces the whole text with `text`, past read-only and without undo, as `append` writes.
    pub fn set_text(&mut self, text: &str) {
        let (read_only, modified) = (std::mem::take(&mut self.read_only), self.modified);
        self.raw_delete(0..self.len_chars())
            .expect("the whole text is in range");
        self.raw_insert(0, text).expect("the start is in range");
        (self.read_only, self.modified) = (read_only, modified);
    }

    /// Applies `edits` as `diff` makes them, past read-only. A marker at an edit's end keeps
    /// the text after it, so one at a line's start stays on that line when lines go in above.
    pub fn replace(&mut self, edits: &[(Range<usize>, String)]) -> Result<(), Error> {
        let read_only = std::mem::take(&mut self.read_only);
        let mut result = Ok(());
        // Back to front, so each range is still where `diff` found it.
        for (range, text) in edits.iter().rev() {
            let after: Vec<MarkerId> = (self.markers.iter())
                .filter(|(_, (pos, _))| *pos == range.end)
                .map(|(&id, _)| id)
                .collect();
            result = self
                .delete(range.clone())
                .and_then(|()| self.insert(range.start, text));
            if result.is_err() {
                break;
            }
            let end = range.start + text.chars().count();
            for id in after {
                self.markers.get_mut(&id).expect("live").0 = end;
            }
        }
        self.read_only = read_only;
        result
    }
}

/// What turns a buffer's text into its file's: the edits, and the file's line ending, BOM and hash.
#[derive(Clone, Debug, PartialEq)]
pub struct Revert {
    pub edits: Vec<(Range<usize>, String)>,
    pub crlf: bool,
    pub bom: bool,
    pub hash: u64,
}

/// What turns `old` into the file at `path`, given its read: its bytes and hash, `None` when
/// there is no file, or why it couldn't be read. Diffing is slow, so services call this.
pub fn revert(
    path: &Path,
    old: &Rope,
    file: Result<Option<(Vec<u8>, u64)>, String>,
) -> Result<Revert, String> {
    let (new, crlf, bom, hash) = disk_text(path, file)?;
    let edits = diff(&old.to_string(), &new);
    Ok(Revert {
        edits,
        crlf,
        bom,
        hash,
    })
}

/// The file at `path` decoded from its read, with its line ending, BOM and hash, or why not.
fn disk_text(
    path: &Path,
    file: Result<Option<(Vec<u8>, u64)>, String>,
) -> Result<(String, bool, bool, u64), String> {
    let (bytes, hash) = file?.ok_or_else(|| format!("File {} no longer exists", path.display()))?;
    let (text, crlf, bom) =
        decode(&bytes).map_err(|reason| format!("{}: {reason}", path.display()))?;
    Ok((text, crlf, bom, hash))
}

/// A unified diff from the file at `path`, given its read as for `revert`, to `text`, the text
/// of the buffer `name`; empty when they don't differ. Diffing is slow, so services call this.
pub fn unified_diff(
    path: &Path,
    name: &str,
    text: &Rope,
    file: Result<Option<(Vec<u8>, u64)>, String>,
) -> Result<String, String> {
    let (disk, ..) = disk_text(path, file)?;
    let text = text.to_string();
    let diff = similar::TextDiff::from_lines(&disk, &text);
    // `similar` writes the header only before a hunk, so no differences give "".
    Ok((diff.unified_diff())
        .header(&path.display().to_string(), name)
        .to_string())
}

/// The changed lines between `old` and `new`, in order: each old char range with its new text.
// shortcut: a marker inside a changed line goes to the hunk's start; diff hunks by chars if that grates.
pub fn diff(old: &str, new: &str) -> Vec<(Range<usize>, String)> {
    let diff = similar::TextDiff::from_lines(old, new);
    let (olds, news) = (diff.old_slices(), diff.new_slices());
    let mut at = 0;
    let mut edits = Vec::new();
    for op in diff.ops() {
        let (tag, old_lines, new_lines) = op.as_tag_tuple();
        let len: usize = olds[old_lines].iter().map(|s| s.chars().count()).sum();
        if tag != similar::DiffTag::Equal {
            edits.push((at..at + len, news[new_lines].concat()));
        }
        at += len;
    }
    edits
}

/// The text as the file's bytes: its BOM and line ending, as loaded.
pub fn encode(text: &Rope, crlf: bool, bom: bool) -> Vec<u8> {
    let mut out = String::with_capacity(text.len_bytes() + 3);
    if bom {
        out.push('\u{feff}');
    }
    for chunk in text.chunks() {
        match crlf {
            true => out.push_str(&chunk.replace('\n', "\r\n")),
            false => out.push_str(chunk),
        }
    }
    out.into_bytes()
}

/// The hash a buffer keeps of its file's bytes, to notice changes on disk. FNV-1a 64, fixed
/// because recovery files store it.
pub fn hash(bytes: &[u8]) -> u64 {
    (bytes.iter()).fold(0xcbf2_9ce4_8422_2325, |h, &b| {
        (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3)
    })
}

/// Validates and prepares a preview on the file service worker, without hashing its bytes.
pub fn prepare_preview(bytes: &[u8]) -> Result<Rope, &'static str> {
    decode(bytes).map(|(text, ..)| Rope::from_str(&text))
}

/// A file's bytes as buffer text, and whether it used CRLF and a BOM; or why it is refused.
pub fn decode(bytes: &[u8]) -> Result<(String, bool, bool), &'static str> {
    let bom = bytes.starts_with(b"\xEF\xBB\xBF");
    if bytes[..bytes.len().min(8192)].contains(&0) {
        return Err("binary file");
    }
    let text =
        std::str::from_utf8(&bytes[if bom { 3 } else { 0 }..]).map_err(|_| "not valid UTF-8")?;
    // The first line break sets the ending; lone CRs stay in the text.
    let crlf = text.find('\n').is_some_and(|i| text[..i].ends_with('\r'));
    let text = match crlf {
        true => text.replace("\r\n", "\n"),
        false => text.to_string(),
    };
    Ok((text, crlf, bom))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn with_text(text: &str) -> (Buffers, BufferId) {
        let mut buffers = Buffers::default();
        let id = buffers.create_for_file(Path::new("/t/a.txt"), text);
        (buffers, id)
    }

    #[test]
    fn prose_and_sessions_wrap_at_words_and_the_rest_at_the_edge() {
        let mut buffers = Buffers::default();
        let mut wrap = |path: Option<&str>, mode| {
            let id = buffers.create("b");
            let b = buffers.get_mut(id).unwrap();
            (b.file, b.major_mode) = (path.map(PathBuf::from), mode);
            b.wrap(5)
        };
        for path in ["/t/a.md", "/t/a.markdown", "/t/a.txt"] {
            assert_eq!(
                wrap(Some(path), "fundamental-mode"),
                Some((4, true)),
                "{path}"
            );
        }
        assert_eq!(
            wrap(None, crate::agents::MODE),
            Some((4, true)),
            "a session"
        );
        assert_eq!(wrap(Some("/t/a.rs"), "fundamental-mode"), Some((4, false)));
        assert_eq!(wrap(None, "fundamental-mode"), Some((4, false)), "no file");
        let (mut buffers, id) = with_text("");
        let b = buffers.get_mut(id).unwrap();
        assert_eq!(b.wrap(1), None, "too narrow");
        b.truncate_lines = true;
        assert_eq!(b.wrap(5), None, "truncating");
        (b.truncate_lines, b.major_mode) = (false, "minibuffer-mode");
        assert_eq!(b.wrap(5), None, "the minibuffer");
    }

    #[test]
    fn names_are_unique_and_order_is_most_recent_first() {
        let mut buffers = Buffers::default();
        let a = buffers.create("x");
        let b = buffers.create_for_file(Path::new("/d/x"), "");
        let c = buffers.create("x");
        let names = |bs: &Buffers| bs.recent().map(|(_, b)| b.name.clone()).collect::<Vec<_>>();
        assert_eq!(names(&buffers), ["x", "x<2>", "x<3>"]);
        assert_eq!(buffers.find("x<2>"), Some(b));
        buffers.raise(c);
        buffers.kill(a).unwrap();
        assert_eq!(names(&buffers), ["x<3>", "x<2>"]);
        assert_eq!(buffers.create("x<2>").0, 3);
        assert_eq!(buffers.get(BufferId(3)).unwrap().name, "x<2><2>");
    }

    #[test]
    fn create_and_look_up() {
        let mut buffers = Buffers::default();
        let id = buffers.create("notes");
        let b = buffers.get(id).unwrap();
        assert_eq!(b.name, "notes");
        assert_eq!(b.text().len_chars(), 0);
        assert_eq!(b.file, None);
    }

    #[test]
    fn ids_are_not_reused_and_killed_buffers_are_gone() {
        let mut buffers = Buffers::default();
        let a = buffers.create("a");
        buffers.kill(a).unwrap();
        assert!(buffers.get(a).is_none());
        assert_eq!(buffers.kill(a).err(), Some(Error::NoSuchBuffer(a)));
        assert_ne!(buffers.create("b"), a);
    }

    #[test]
    fn buffer_for_a_file() {
        let mut buffers = Buffers::default();
        let path = Path::new("/tmp/proj/src/main.rs");
        let id = buffers.create_for_file(path, "fn main() {}");
        let b = buffers.get(id).unwrap();
        assert_eq!(b.file.as_deref(), Some(path));
        assert_eq!(b.name, "main.rs");
        assert_eq!(b.text(), "fn main() {}");
        assert!(!b.modified());
    }

    #[test]
    fn edits_by_char_offset() {
        let (mut buffers, id) = with_text("wrld");
        let b = buffers.get_mut(id).unwrap();
        b.insert(2, "ö").unwrap();
        assert_eq!(b.text(), "wröld");

        let (mut buffers, id) = with_text("héllo");
        let b = buffers.get_mut(id).unwrap();
        b.delete(1..3).unwrap();
        assert_eq!(b.text(), "hlo");
    }

    #[test]
    fn invalid_edits_change_nothing() {
        let (mut buffers, id) = with_text("hello");
        let b = buffers.get_mut(id).unwrap();
        assert_eq!(b.insert(6, "x"), Err(Error::OutOfRange));
        assert_eq!(b.delete(3..9), Err(Error::OutOfRange));
        #[allow(clippy::reversed_empty_ranges)]
        let reversed = 3..1;
        assert_eq!(b.delete(reversed), Err(Error::OutOfRange));
        assert_eq!(b.text(), "hello");
        assert!(!b.modified());
    }

    #[test]
    fn hash_is_fnv1a() {
        assert_eq!(hash(b""), 0xcbf2_9ce4_8422_2325);
        assert_eq!(hash(b"a"), 0xaf63_dc4c_8601_ec8c);
    }

    #[test]
    fn decodes_and_encodes_bom_and_line_endings() {
        assert_eq!(decode(b"a\nb\r\n"), Ok(("a\nb\r\n".into(), false, false)));
        assert_eq!(decode(b"a\r\nb\r\n"), Ok(("a\nb\n".into(), true, false)));
        assert_eq!(decode(b"\xEF\xBB\xBFhi"), Ok(("hi".into(), false, true)));
        assert_eq!(decode(b""), Ok((String::new(), false, false)));
        assert_eq!(decode(b"a\0b"), Err("binary file"));
        assert_eq!(decode(b"caf\xE9"), Err("not valid UTF-8"));
        // A NUL past the first 8 KB is valid UTF-8 text.
        let late = [vec![b'a'; 8192], vec![0]].concat();
        assert!(decode(&late).is_ok());

        let (mut buffers, id) = with_text("a\nb\n");
        let b = buffers.get_mut(id).unwrap();
        (b.crlf, b.bom) = (true, true);
        assert_eq!(encode(b.text(), true, true), b"\xEF\xBB\xBFa\r\nb\r\n");
    }

    #[test]
    fn read_only_buffers_refuse_edits() {
        let (mut buffers, id) = with_text("x");
        let b = buffers.get_mut(id).unwrap();
        b.read_only = true;
        assert_eq!(b.insert(0, "y"), Err(Error::ReadOnly));
        assert_eq!(b.delete(0..1), Err(Error::ReadOnly));
        assert_eq!(b.text(), "x");
    }

    #[test]
    fn text_before_the_prompt_is_read_only_and_output_keeps_undo() {
        let (mut buffers, id) = with_text("> ");
        let b = buffers.get_mut(id).unwrap();
        b.prompt = Some(b.add_marker(2, InsertionType::Stay).unwrap());
        assert_eq!(b.insert(1, "x"), Err(Error::TextReadOnly));
        assert_eq!(b.delete(1..3), Err(Error::TextReadOnly));
        b.insert(2, "ab").unwrap();
        assert_eq!(b.prompt_start(), 2);
        b.undo_boundary(4);
        b.delete(3..4).unwrap();
        b.insert_output(0, "hi\n\n");
        assert_eq!(
            (b.text().to_string(), b.prompt_start()),
            ("hi\n\n> a".into(), 6)
        );
        assert_eq!(b.undo(), Some(8));
        assert_eq!(b.text(), "hi\n\n> ab");
        assert_eq!(b.redo(), Some(7));
        assert_eq!(b.text(), "hi\n\n> a");
        // Replacing output with longer text moves the history with the prompt area.
        b.replace_output(0..2, "hello");
        assert_eq!(b.undo(), Some(11));
        assert_eq!(b.text(), "hello\n\n> ab");
    }

    #[test]
    fn append_passes_read_only_without_undo() {
        let (mut buffers, id) = with_text("x");
        let b = buffers.get_mut(id).unwrap();
        b.read_only = true;
        b.append("y\n");
        assert_eq!(b.text(), "xy\n");
        assert!(b.read_only);
        assert_eq!(b.undo(), None);
    }

    #[test]
    fn modified_flag() {
        let (mut buffers, id) = with_text("");
        let b = buffers.get_mut(id).unwrap();
        b.insert(0, "").unwrap();
        b.delete(0..0).unwrap();
        assert!(!b.modified());
        b.insert(0, "x").unwrap();
        assert!(b.modified());
        b.set_saved();
        assert!(!b.modified());
        assert_eq!(b.text(), "x");
    }

    #[test]
    fn markers_are_added_moved_and_removed() {
        let (mut buffers, id) = with_text("hello");
        let b = buffers.get_mut(id).unwrap();
        let m = b.add_marker(2, InsertionType::Stay).unwrap();
        assert_eq!(b.marker(m), Some(2));
        assert_eq!(b.add_marker(6, InsertionType::Stay), Err(Error::OutOfRange));
        assert_eq!(b.set_marker(m, 6), Err(Error::OutOfRange));
        b.set_marker(m, 5).unwrap();
        assert_eq!(b.marker(m), Some(5));
        b.remove_marker(m);
        assert_eq!(b.marker(m), None);
        b.set_marker(m, 1).unwrap();
        assert_eq!(b.marker(m), None);
    }

    /// Markers at each of `at`, of both insertion types, after `edit`: (stay, advance) per marker.
    fn markers_after(
        text: &str,
        at: &[usize],
        edit: impl FnOnce(&mut Buffer),
    ) -> Vec<(usize, usize)> {
        let (mut buffers, id) = with_text(text);
        let b = buffers.get_mut(id).unwrap();
        let ms: Vec<_> = at
            .iter()
            .map(|&p| {
                let stay = b.add_marker(p, InsertionType::Stay).unwrap();
                (stay, b.add_marker(p, InsertionType::Advance).unwrap())
            })
            .collect();
        edit(b);
        ms.iter()
            .map(|&(s, a)| (b.marker(s).unwrap(), b.marker(a).unwrap()))
            .collect()
    }

    #[test]
    fn inserts_move_markers_by_insertion_type() {
        let moved = markers_after("abcd", &[1, 2, 3], |b| b.insert(2, "XY").unwrap());
        assert_eq!(moved, [(1, 1), (2, 4), (5, 5)]);
        let moved = markers_after("añb", &[1, 2], |b| b.insert(1, "🙂").unwrap());
        assert_eq!(moved, [(1, 2), (3, 3)]);
    }

    #[test]
    fn deletes_move_markers_to_the_range_start() {
        let moved = markers_after("abcde", &[0, 1, 2, 3, 4], |b| b.delete(1..3).unwrap());
        assert_eq!(moved, [(0, 0), (1, 1), (1, 1), (1, 1), (2, 2)]);
        let moved = markers_after("abcdefgh", &[3], |b| b.delete(1..7).unwrap());
        assert_eq!(
            moved,
            [(1, 1)],
            "a marker inside a range longer than its offset"
        );
    }

    #[test]
    fn refused_edits_leave_markers() {
        let moved = markers_after("hello", &[3], |b| {
            b.insert(6, "x").unwrap_err();
            b.delete(2..9).unwrap_err();
        });
        assert_eq!(moved, [(3, 3)]);
    }

    proptest::proptest! {
        /// Between `<` and `>`, whatever is inserted there, a stay marker keeps right after `<`
        /// and an advance marker right before `>`.
        #[test]
        fn markers_follow_edits_elsewhere(
            edits in proptest::collection::vec((proptest::bool::ANY, 0..20usize, 0..4usize, "[a-zé]{0,3}"), 0..30),
        ) {
            let (mut buffers, id) = with_text("ab<>cd");
            let b = buffers.get_mut(id).unwrap();
            let stay = b.add_marker(3, InsertionType::Stay).unwrap();
            let advance = b.add_marker(3, InsertionType::Advance).unwrap();
            let find = |b: &Buffer, s| b.text().chars().position(|c| c == s).unwrap();
            for (insert, at, len, text) in edits {
                let at = at % (b.len_chars() + 1);
                if insert {
                    b.insert(at, &text).unwrap();
                } else {
                    let end = (at + len).min(b.len_chars());
                    // The sentinels stay so there is something to check against.
                    if !b.text().slice(at..end).chars().any(|c| c == '<' || c == '>') {
                        b.delete(at..end).unwrap();
                    }
                }
                proptest::prop_assert_eq!(b.marker(stay), Some(find(b, '<') + 1));
                proptest::prop_assert_eq!(b.marker(advance), Some(find(b, '>')));
            }
        }

        /// Undo all restores the original text, redo all the final one, and markers never leave the text.
        #[test]
        fn undo_redo_round_trip_random_edits(
            start in "[a-zé🙂\n]{0,8}",
            markers in proptest::collection::vec((0..20usize, proptest::bool::ANY), 0..4),
            ops in proptest::collection::vec((0..3u8, 0..20usize, 0..4usize, "[a-zé🙂\n]{0,3}"), 0..30),
        ) {
            let (mut buffers, id) = with_text(&start);
            let b = buffers.get_mut(id).unwrap();
            let ms: Vec<_> = markers
                .iter()
                .map(|&(at, advance)| {
                    let kind = if advance { InsertionType::Advance } else { InsertionType::Stay };
                    b.add_marker(at % (b.len_chars() + 1), kind).unwrap()
                })
                .collect();
            let in_bounds = |b: &Buffer| ms.iter().all(|&m| b.marker(m).unwrap() <= b.len_chars());
            for (op, at, len, text) in ops {
                let at = at % (b.len_chars() + 1);
                match op {
                    0 => b.insert(at, &text).unwrap(),
                    1 => b.delete(at..(at + len).min(b.len_chars())).unwrap(),
                    _ => b.undo_boundary(at),
                }
                proptest::prop_assert!(in_bounds(b));
            }
            let end = b.text().to_string();
            while b.undo().is_some() {
                proptest::prop_assert!(in_bounds(b));
            }
            proptest::prop_assert_eq!(b.text().to_string(), start);
            while b.redo().is_some() {
                proptest::prop_assert!(in_bounds(b));
            }
            proptest::prop_assert_eq!(b.text().to_string(), end);
        }
    }

    #[test]
    fn char_idx_clamps_before_the_newline() {
        let (buffers, id) = with_text("ab\ncd");
        let b = buffers.get(id).unwrap();
        assert_eq!(b.char_idx(0, 9), Ok(2), "a line with LF");
        assert_eq!(b.char_idx(1, 9), Ok(5), "a last line without");
        let (buffers, id) = with_text("ab\n");
        assert_eq!(
            buffers.get(id).unwrap().char_idx(1, 9),
            Ok(3),
            "an empty last line"
        );
    }

    #[test]
    fn properties_near_a_range() {
        let (mut buffers, id) = with_text(&"x".repeat(100));
        let b = buffers.get_mut(id).unwrap();
        let face = |n| {
            Prop::Face(Face {
                fg: Some(n),
                ..Face::default()
            })
        };
        for start in (0..100).step_by(10) {
            b.add_property(start..start + 5, face(start as u32))
                .unwrap();
        }
        let near = |b: &Buffer, r| -> Vec<_> {
            b.properties_near(r).into_iter().map(|(r, _)| r).collect()
        };
        assert_eq!(near(b, 23..41), [20..25, 30..35, 40..45]);
        assert_eq!(near(b, 25..29), [20..25], "touching its end");
        b.insert(0, "yy").unwrap();
        b.delete(50..60).unwrap();
        assert_eq!(
            near(b, 40..52),
            [42..47, 52..57],
            "after edits, the emptied one gone"
        );
        // Removing keeps the rest in order.
        b.remove_properties(40..48);
        assert_eq!(near(b, 40..52), [52..57]);
        // A long overlapping one makes the list unsorted by end; the full scan still finds it.
        b.add_property(0..90, face(1)).unwrap();
        assert_eq!(near(b, 68..70), [0..90]);
    }

    #[test]
    fn nested_properties_with_ascending_ends_bisect() {
        let (mut buffers, id) = with_text("abcdefghij");
        let b = buffers.get_mut(id).unwrap();
        let face = Prop::Face(Face::default());
        for r in [1..3, 1..6, 4..6, 5..9] {
            b.add_property(r, face.clone()).unwrap();
        }
        assert!(b.props_sorted);
        let near = |b: &Buffer, r| -> Vec<_> {
            b.properties_near(r).into_iter().map(|(r, _)| r).collect()
        };
        assert_eq!(near(b, 4..4), [1..6, 4..6]);
        // One added in the middle goes to its place and keeps the order.
        b.add_property(3..6, face.clone()).unwrap();
        assert!(b.props_sorted);
        assert_eq!(near(b, 3..3), [1..3, 1..6, 3..6]);
        b.remove_properties(3..6);
        b.remove_properties(0..6);
        assert_eq!(
            near(b, 0..10),
            [5..9],
            "only the one past the range is left"
        );
    }

    #[test]
    fn properties_follow_edits() {
        let (mut buffers, id) = with_text("abcdef");
        let b = buffers.get_mut(id).unwrap();
        let face = Prop::Face(Face::default());
        b.add_property(2..4, face.clone()).unwrap();
        for at in [3, 2, 6] {
            b.insert(at, "X").unwrap();
        }
        assert_eq!(b.properties().collect::<Vec<_>>(), [(3..6, &face)]);
        b.delete(1..7).unwrap();
        assert_eq!(b.properties().count(), 0, "deleted whole");
        assert_eq!(b.add_property(1..9, face), Err(Error::OutOfRange));
    }

    #[test]
    fn undo_and_redo_groups() {
        let (mut buffers, id) = with_text("");
        let b = buffers.get_mut(id).unwrap();
        assert_eq!(b.undo(), None);
        b.undo_boundary(0);
        b.insert(0, "ab").unwrap();
        b.insert(2, "c").unwrap();
        b.undo_boundary(3);
        b.insert(9, "x").unwrap_err();
        b.insert(3, "").unwrap();
        b.delete(1..1).unwrap();
        b.undo_boundary(3);
        b.delete(0..1).unwrap();
        b.undo_boundary(0);
        assert_eq!(b.text(), "bc");
        assert_eq!(b.undo(), Some(3), "point before the delete");
        assert_eq!(b.text(), "abc");
        assert_eq!(b.undo(), Some(0), "refused and empty edits made no group");
        assert_eq!(b.text(), "");
        assert_eq!(b.undo(), None);
        assert_eq!(b.redo(), Some(3));
        assert_eq!(b.redo(), Some(0));
        assert_eq!(b.text(), "bc");
        assert_eq!(b.redo(), None);
        b.undo();
        b.insert(0, "z").unwrap();
        assert_eq!(b.redo(), None, "a new edit clears redo");
        assert_eq!(b.text(), "zabc");
    }

    #[test]
    fn undo_without_boundaries_uses_the_edit_position() {
        let (mut buffers, id) = with_text("héllo");
        let b = buffers.get_mut(id).unwrap();
        b.delete(1..3).unwrap();
        assert_eq!(b.undo(), Some(1));
        assert_eq!(b.text(), "héllo");
        assert_eq!(b.redo(), Some(1));
        assert_eq!(b.text(), "hlo");
    }

    #[test]
    fn line_col_round_trips_in_non_ascii_text() {
        let (buffers, id) = with_text("añb\nçd🙂e");
        let b = buffers.get(id).unwrap();
        assert_eq!(b.line_col(8), Ok((1, 4)));
        assert_eq!(b.line_col(9), Err(Error::OutOfRange));
        assert_eq!(b.char_idx(1, 2), Ok(6));
        for idx in 0..=b.len_chars() {
            let (line, col) = b.line_col(idx).unwrap();
            assert_eq!(b.char_idx(line, col), Ok(idx));
        }
    }

    #[test]
    fn column_clamps_and_lines_past_the_end_fail() {
        let (buffers, id) = with_text("ab\ncd");
        let b = buffers.get(id).unwrap();
        assert_eq!(b.char_idx(0, 99), Ok(2));
        assert_eq!(b.char_idx(1, 99), Ok(5));
        assert_eq!(b.char_idx(5, 0), Err(Error::OutOfRange));
        assert_eq!(b.char_idx(2, 0), Err(Error::OutOfRange));
    }

    #[test]
    fn only_lf_breaks_lines() {
        let (buffers, id) = with_text("a\rb\u{2028}c\r\nd");
        let b = buffers.get(id).unwrap();
        assert_eq!(b.len_lines(), 2);
        assert_eq!(b.line_col(7), Ok((1, 0)));
    }

    /// `old` with `diff`'s edits to `new` applied, and where a marker at `at` ends up.
    fn replaced(old: &str, new: &str, at: usize) -> (String, usize) {
        let (mut buffers, id) = with_text(old);
        let b = buffers.get_mut(id).unwrap();
        let m = b.add_marker(at, InsertionType::Stay).unwrap();
        b.replace(&diff(old, new)).unwrap();
        (b.text().to_string(), b.marker(m).unwrap())
    }

    proptest::proptest! {
        #[test]
        fn a_diff_turns_the_old_text_into_the_new(
            old in "[ab\\n\\ré]{0,12}",
            new in "[ab\\n\\ré]{0,12}",
        ) {
            proptest::prop_assert_eq!(replaced(&old, &new, 0).0, new);
        }
    }

    #[test]
    fn markers_on_unchanged_lines_keep_their_line_and_column() {
        let old = "a\nb\nc\nd\n";
        // Lines inserted right above "c", and "a" replaced: "c" at column 0 stays on "c".
        assert_eq!(
            replaced(old, "A\nb\nx\ny\nc\nd\n", 4),
            ("A\nb\nx\ny\nc\nd\n".into(), 8)
        );
        // A marker inside a changed line goes to the start of its replacement.
        assert_eq!(replaced(old, "a\nB\nc\nd\n", 3).1, 2);
        // The end of the text stays the end.
        assert_eq!(replaced("a\nb", "a\nbc", 3).1, 4);
    }

    #[test]
    fn replace_is_one_undoable_edit_past_read_only() {
        let (mut buffers, id) = with_text("a\nb\n");
        let b = buffers.get_mut(id).unwrap();
        b.read_only = true;
        b.undo_boundary(0);
        b.replace(&diff("a\nb\n", "x\nb\ny\n")).unwrap();
        assert!(b.read_only);
        b.undo_boundary(0);
        b.read_only = false;
        b.undo();
        assert_eq!(b.text().to_string(), "a\nb\n");
    }

    #[test]
    fn unified_diff_runs_from_the_file_to_the_buffer() {
        let path = Path::new("/t/a.txt");
        let file = |text: &str| Ok(Some((text.as_bytes().to_vec(), 0)));
        let text = Rope::from_str("a\nb\nc");
        assert_eq!(
            unified_diff(path, "a.txt", &text, file("a\r\nB\r\nc\r\n")).unwrap(),
            "--- /t/a.txt\n+++ a.txt\n@@ -1,3 +1,3 @@\n a\n-B\n-c\n+b\n+c\n\\ No newline at end of file\n"
        );
        assert_eq!(
            unified_diff(path, "a.txt", &text, file("a\nb\nc")),
            Ok("".into())
        );
        let gone = unified_diff(path, "a.txt", &text, Ok(None));
        assert_eq!(gone.unwrap_err(), "File /t/a.txt no longer exists");
    }

    #[test]
    fn set_text_replaces_everything_past_read_only_without_undo() {
        let (mut buffers, id) = with_text("a\nb\n");
        let b = buffers.get_mut(id).unwrap();
        b.read_only = true;
        b.set_text("x");
        assert_eq!(b.text().to_string(), "x");
        assert!(b.read_only && !b.modified());
        assert_eq!(b.undo(), None);
    }

    #[test]
    fn revert_decodes_the_file_or_says_why_not() {
        let path = Path::new("/t/a.txt");
        let old = Rope::from_str("a\n");
        let r = revert(
            path,
            &old,
            Ok(Some((b"\xEF\xBB\xBFa\r\nb\r\n".to_vec(), 7))),
        )
        .unwrap();
        assert_eq!(
            (r.edits, r.crlf, r.bom, r.hash),
            (vec![(2..2, "b\n".into())], true, true, 7)
        );
        let err = |file| revert(path, &old, file).unwrap_err();
        assert_eq!(err(Ok(None)), "File /t/a.txt no longer exists");
        assert_eq!(err(Ok(Some((b"a\0".to_vec(), 0)))), "/t/a.txt: binary file");
        assert_eq!(err(Err("/t/a.txt: denied".into())), "/t/a.txt: denied");
    }
}
