//! Buffers: content addressed by id, edited by char offset.

use std::collections::BTreeMap;
use std::fmt;
use std::ops::Range;
use std::path::{Path, PathBuf};

use ropey::Rope;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BufferId(u64);

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
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Error::NoSuchBuffer(id) => write!(f, "no buffer {}", id.0),
            Error::OutOfRange => f.write_str("position out of range"),
        }
    }
}

impl std::error::Error for Error {}

impl From<ropey::Error> for Error {
    fn from(_: ropey::Error) -> Self {
        Error::OutOfRange
    }
}

/// Every buffer in the core. Ids only increase, so a stale id never finds a newer buffer.
#[derive(Default)]
pub struct Buffers {
    next: u64,
    map: BTreeMap<BufferId, Buffer>,
}

impl Buffers {
    pub fn create(&mut self, name: &str) -> BufferId {
        self.add(Buffer {
            name: name.to_string(),
            content: Content::Text(Rope::new()),
            file: None,
            modified: false,
            markers: BTreeMap::new(),
            next_marker: 0,
        })
    }

    /// A buffer visiting `path`, holding `text` already read from it.
    pub fn create_for_file(&mut self, path: &Path, text: &str) -> BufferId {
        let name = path.file_name().unwrap_or(path.as_os_str());
        self.add(Buffer {
            name: name.to_string_lossy().into_owned(),
            content: Content::Text(Rope::from_str(text)),
            file: Some(path.to_path_buf()),
            modified: false,
            markers: BTreeMap::new(),
            next_marker: 0,
        })
    }

    fn add(&mut self, buffer: Buffer) -> BufferId {
        let id = BufferId(self.next);
        self.next += 1;
        self.map.insert(id, buffer);
        id
    }

    pub fn get(&self, id: BufferId) -> Option<&Buffer> {
        self.map.get(&id)
    }

    pub fn get_mut(&mut self, id: BufferId) -> Option<&mut Buffer> {
        self.map.get_mut(&id)
    }

    pub fn kill(&mut self, id: BufferId) -> Result<Buffer, Error> {
        self.map.remove(&id).ok_or(Error::NoSuchBuffer(id))
    }
}

/// The content backend; the terminal grid becomes a second variant.
pub enum Content {
    Text(Rope),
}

pub struct Buffer {
    pub name: String,
    content: Content,
    pub file: Option<PathBuf>,
    modified: bool,
    // shortcut: every edit walks all markers, fine for a few per buffer; index them if buffers get hundreds.
    markers: BTreeMap<MarkerId, (usize, InsertionType)>,
    next_marker: u64,
}

impl Buffer {
    pub fn text(&self) -> &Rope {
        match &self.content {
            Content::Text(rope) => rope,
        }
    }

    pub fn insert(&mut self, char_idx: usize, text: &str) -> Result<(), Error> {
        match &mut self.content {
            Content::Text(rope) => rope.try_insert(char_idx, text)?,
        }
        let n = text.chars().count();
        for (pos, kind) in self.markers.values_mut() {
            if *pos > char_idx || (*pos == char_idx && *kind == InsertionType::Advance) {
                *pos += n;
            }
        }
        self.modified |= !text.is_empty();
        Ok(())
    }

    pub fn delete(&mut self, range: Range<usize>) -> Result<(), Error> {
        let changed = !range.is_empty();
        match &mut self.content {
            Content::Text(rope) => rope.try_remove(range.clone())?,
        }
        for (pos, _) in self.markers.values_mut() {
            if *pos > range.start {
                *pos = range.start.max(*pos - range.len());
            }
        }
        self.modified |= changed;
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
        let len = slice.len_chars() - usize::from(slice.chars().last() == Some('\n'));
        Ok(start + col.min(len))
    }

    pub fn modified(&self) -> bool {
        self.modified
    }

    pub fn set_saved(&mut self) {
        self.modified = false;
    }
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
}
