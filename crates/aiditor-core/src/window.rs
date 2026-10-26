//! Windows: viewports onto buffers, holding point and mark as markers and the first line shown.

use crate::buffer::{Buffer, BufferId, Buffers, Error, InsertionType, MarkerId};
use crate::display::{self, Rect};

/// How many marks the ring keeps, Emacs's `mark-ring-max`.
const MARK_RING_MAX: usize = 16;

pub struct Window {
    buffer: BufferId,
    point: MarkerId,
    mark: Option<MarkerId>,
    /// Whether the region shows, as in Emacs's transient-mark-mode.
    active: bool,
    /// Earlier marks, most recent first.
    ring: Vec<MarkerId>,
    /// The first line shown; moves only to keep point's row visible.
    top: usize,
    /// The first row shown of `top` when it wraps.
    top_row: usize,
    /// The lines shown at the last redisplay, 1 before the first.
    rows: usize,
    /// The columns shown at the last redisplay, which long lines wrap at.
    cols: usize,
    /// The first column shown of point's line; moves only to keep point visible.
    left: usize,
}

impl Window {
    /// A window on `buffer` with point where the last window to show it left it.
    pub fn new(buffers: &mut Buffers, buffer: BufferId) -> Self {
        let b = buffers.get_mut(buffer).expect("window buffer is live");
        let at = b.marker(b.point).expect("a buffer's point is live");
        Window {
            buffer,
            // Stay, like Emacs's window-point: typing commands move point themselves.
            point: (b.add_marker(at, InsertionType::Stay)).expect("the buffer's point is in it"),
            mark: None,
            active: false,
            ring: Vec::new(),
            top: 0,
            top_row: 0,
            rows: 1,
            cols: 0,
            left: 0,
        }
    }

    pub fn buffer(&self) -> BufferId {
        self.buffer
    }

    /// A new window on the same buffer with the same point, first line and rows, and no mark.
    pub fn split(&self, buffers: &mut Buffers) -> Self {
        let w = Window {
            top: self.top,
            top_row: self.top_row,
            rows: self.rows,
            cols: self.cols,
            left: self.left,
            ..Window::new(buffers, self.buffer)
        };
        let point = self.point(buffers);
        w.set_point(buffers, point).expect("point is in the buffer");
        w
    }

    /// Removes this window's markers from its buffer, as when it is deleted, leaving the
    /// buffer's point at this window's.
    pub fn release(&self, buffers: &mut Buffers) {
        if let Some(old) = buffers.get_mut(self.buffer) {
            let point = old.marker(self.point).expect("window point is live");
            old.set_marker(old.point, point)
                .expect("point is in the buffer");
            old.remove_marker(self.point);
            for &m in self.mark.iter().chain(&self.ring) {
                old.remove_marker(m);
            }
        }
    }

    /// Shows `buffer` instead, with point at the buffer's point; the old buffer keeps no markers
    /// of ours.
    pub fn show(&mut self, buffers: &mut Buffers, buffer: BufferId) {
        self.release(buffers);
        let (rows, cols) = (self.rows, self.cols);
        *self = Window {
            rows,
            cols,
            ..Window::new(buffers, buffer)
        };
    }

    pub fn point(&self, buffers: &Buffers) -> usize {
        buffers
            .get(self.buffer)
            .and_then(|b| b.marker(self.point))
            .expect("window point is live")
    }

    pub fn set_point(&self, buffers: &mut Buffers, char_idx: usize) -> Result<(), Error> {
        self.live(buffers)?.set_marker(self.point, char_idx)
    }

    /// The mark, or `None` until a command sets it.
    pub fn mark(&self, buffers: &Buffers) -> Option<usize> {
        self.mark.and_then(|m| buffers.get(self.buffer)?.marker(m))
    }

    pub fn set_mark(&mut self, buffers: &mut Buffers, char_idx: usize) -> Result<(), Error> {
        let buffer = self.live(buffers)?;
        match self.mark {
            Some(m) => buffer.set_marker(m, char_idx),
            None => {
                self.mark = Some(buffer.add_marker(char_idx, InsertionType::Stay)?);
                Ok(())
            }
        }
    }

    /// Sets the mark at `char_idx`, pushing the old one onto the mark ring.
    pub fn push_mark(
        &mut self,
        buffers: &mut Buffers,
        char_idx: usize,
        activate: bool,
    ) -> Result<(), Error> {
        let buffer = buffers
            .get_mut(self.buffer)
            .ok_or(Error::NoSuchBuffer(self.buffer))?;
        let new = buffer.add_marker(char_idx, InsertionType::Stay)?;
        if let Some(old) = self.mark.replace(new) {
            self.ring.insert(0, old);
            if self.ring.len() > MARK_RING_MAX {
                buffer.remove_marker(self.ring.pop().expect("ring is full"));
            }
        }
        self.active = activate;
        Ok(())
    }

    /// Makes the ring's first mark the mark and puts the old one at the ring's end, deactivated.
    pub fn pop_mark(&mut self) {
        if let Some(old) = self.mark
            && !self.ring.is_empty()
        {
            self.ring.push(old);
            self.mark = Some(self.ring.remove(0));
        }
        self.active = false;
    }

    /// The mark ring's positions, most recent first.
    pub fn ring(&self, buffers: &Buffers) -> Vec<usize> {
        let buffer = buffers.get(self.buffer).expect("window buffer is live");
        self.ring.iter().filter_map(|&m| buffer.marker(m)).collect()
    }

    /// Whether the mark is set and active.
    pub fn active(&self) -> bool {
        self.active && self.mark.is_some()
    }

    pub fn set_active(&mut self, active: bool) {
        self.active = active;
    }

    /// Moves the first row just far enough that point's row is among the `rows` shown, and
    /// returns the first line.
    pub fn scroll(&mut self, buffers: &Buffers, rows: usize) -> usize {
        let buffer = buffers.get(self.buffer).expect("window buffer is live");
        let (line, col) = buffer.line_col(self.point(buffers)).unwrap_or_default();
        self.rows = rows.max(1);
        let starts = |l: usize| self.row_starts(buffer, l);
        let row = starts(line).partition_point(|&s| s <= col) - 1;
        // The lowest first row: `rows - 1` rows above point's.
        let (mut low, mut low_row) = (line, row);
        for _ in 1..self.rows {
            match (low_row, low) {
                (0, 0) => break,
                (0, _) => (low, low_row) = (low - 1, starts(low - 1).len() - 1),
                _ => low_row -= 1,
            }
        }
        if self.top_row > 0 && self.top < buffer.len_lines() {
            self.top_row = self.top_row.min(starts(self.top).len() - 1);
        }
        (self.top, self.top_row) = (self.top, self.top_row).clamp((low, low_row), (line, row));
        self.top
    }

    /// Where each visual row of `buffer`'s `line` starts at the last redisplay's width, `[0]`
    /// when it truncates.
    // shortcut: rows are counted in buffer chars, so a fold on a wrapped line makes it count more rows than it shows; count display chars when folds get common.
    pub(crate) fn row_starts(&self, buffer: &Buffer, line: usize) -> Vec<usize> {
        let Some((width, word)) = buffer.wrap(self.cols) else {
            return vec![0];
        };
        let text = buffer.text().line(line);
        let end = buffer
            .char_idx(line, usize::MAX)
            .expect("line is in the buffer");
        let len = end - buffer.text().line_to_char(line);
        display::row_starts(len, width, word, |i| text.char(i).is_whitespace())
    }

    /// The first row shown of the first line at the last `scroll`.
    pub fn top_row(&self) -> usize {
        self.top_row
    }

    /// Sets the columns long lines wrap at, as shown at this redisplay.
    pub(crate) fn set_cols(&mut self, cols: usize) {
        self.cols = cols;
    }

    /// Moves point to `char_idx`, centering its line when it is not among the rows shown.
    pub fn jump(&mut self, buffers: &mut Buffers, char_idx: usize) -> Result<(), Error> {
        self.set_point(buffers, char_idx)?;
        let buffer = buffers.get(self.buffer).expect("window buffer is live");
        let (line, _) = buffer.line_col(char_idx)?;
        if !(self.top..self.top + self.rows).contains(&line) {
            (self.top, self.top_row) = (line.saturating_sub(self.rows / 2), 0);
        }
        Ok(())
    }

    /// Shows from `line` on; the next `scroll` moves it if point's line is not shown.
    pub fn set_top(&mut self, line: usize) {
        (self.top, self.top_row) = (line, 0);
    }

    pub fn rows(&self) -> usize {
        self.rows
    }

    /// Moves the first column shown just far enough that `col` is among the `cols` shown, 0 while
    /// it fits from column 0, and returns it; a window with no columns does not scroll.
    pub fn hscroll(&mut self, col: usize, cols: usize) -> usize {
        self.left = match cols == 0 || col < cols {
            true => 0,
            false => self.left.clamp(col + 1 - cols, col),
        };
        self.left
    }

    /// The first column shown of point's line at the last redisplay.
    pub fn left(&self) -> usize {
        self.left
    }

    fn live<'a>(&self, buffers: &'a mut Buffers) -> Result<&'a mut Buffer, Error> {
        buffers
            .get_mut(self.buffer)
            .ok_or(Error::NoSuchBuffer(self.buffer))
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub struct WindowId(pub usize);

/// The frame's windows as a binary split tree.
#[derive(Debug)]
pub enum Tree {
    Leaf(WindowId),
    Split {
        /// Side by side when true, one above the other when false.
        side_by_side: bool,
        /// The share of the space the first part takes.
        ratio: f32,
        first: Box<Tree>,
        second: Box<Tree>,
    },
}

impl Tree {
    /// Splits the leaf `id` in half, `new` going below or right of it.
    pub fn split(&mut self, id: WindowId, new: WindowId, side_by_side: bool) {
        match self {
            Tree::Leaf(leaf) if *leaf == id => {
                *self = Tree::Split {
                    side_by_side,
                    ratio: 0.5,
                    first: Box::new(Tree::Leaf(id)),
                    second: Box::new(Tree::Leaf(new)),
                }
            }
            Tree::Leaf(_) => {}
            Tree::Split { first, second, .. } => {
                first.split(id, new, side_by_side);
                second.split(id, new, side_by_side);
            }
        }
    }

    /// Removes the leaf `id`, its sibling taking its parent's place, and returns the sibling's
    /// first leaf; `None` if `id` is the root or not in the tree.
    pub fn remove(&mut self, id: WindowId) -> Option<WindowId> {
        let Tree::Split { first, second, .. } = self else {
            return None;
        };
        let sibling = match (&**first, &**second) {
            (Tree::Leaf(l), _) if *l == id => std::mem::replace(&mut **second, Tree::Leaf(id)),
            (_, Tree::Leaf(l)) if *l == id => std::mem::replace(&mut **first, Tree::Leaf(id)),
            _ => return first.remove(id).or_else(|| second.remove(id)),
        };
        *self = sibling;
        Some(self.leaves()[0])
    }

    /// The leaves top to bottom, left to right.
    pub fn leaves(&self) -> Vec<WindowId> {
        match self {
            Tree::Leaf(id) => vec![*id],
            Tree::Split { first, second, .. } => [first.leaves(), second.leaves()].concat(),
        }
    }

    /// Each leaf's part of `rect`, in `leaves` order, split edges snapped to whole `cell`s
    /// (width, height); a zero cell side is not snapped.
    pub fn layout(&self, rect: Rect, cell: (f32, f32)) -> Vec<(WindowId, Rect)> {
        let snap = |v: f32, cell: f32| {
            if cell > 0. {
                (v / cell).round() * cell
            } else {
                v
            }
        };
        match self {
            Tree::Leaf(id) => vec![(*id, rect)],
            Tree::Split {
                side_by_side,
                ratio,
                first,
                second,
            } => {
                let (mut a, mut b) = (rect, rect);
                if *side_by_side {
                    a.width = snap(rect.width * ratio, cell.0);
                    b.x += a.width;
                    b.width -= a.width;
                } else {
                    a.height = snap(rect.height * ratio, cell.1);
                    b.y += a.height;
                    b.height -= a.height;
                }
                [first.layout(a, cell), second.layout(b, cell)].concat()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_tree_splits_lays_out_and_removes() {
        let w = WindowId;
        let mut tree = Tree::Leaf(w(0));
        assert_eq!(tree.remove(w(0)), None, "the sole window stays");
        tree.split(w(0), w(1), false);
        tree.split(w(0), w(2), false);
        let frame = Rect {
            x: 0.,
            y: 0.,
            width: 80.,
            height: 100.,
        };
        // A frame 10 cells high: halves round to whole cells.
        let cell = (10., 10.);
        let heights: Vec<_> = tree
            .layout(frame, cell)
            .iter()
            .map(|(id, r)| (*id, r.y, r.height))
            .collect();
        assert_eq!(
            heights,
            [(w(0), 0., 30.), (w(2), 30., 20.), (w(1), 50., 50.)]
        );
        tree.split(w(1), w(3), true);
        let lower: Vec<_> = tree.layout(frame, cell)[2..]
            .iter()
            .map(|(_, r)| (r.x, r.width))
            .collect();
        assert_eq!(lower, [(0., 40.), (40., 40.)]);
        assert_eq!(tree.remove(w(0)), Some(w(2)));
        assert_eq!(tree.leaves(), [w(2), w(1), w(3)]);
        assert_eq!(tree.layout(frame, cell)[0].1.height, 50.);
        assert_eq!(tree.remove(w(1)), Some(w(3)));
        assert_eq!(tree.remove(w(2)), Some(w(3)));
        assert_eq!(tree.leaves(), [w(3)]);
    }

    fn window_on(text: &str) -> (Buffers, Window) {
        let mut buffers = Buffers::default();
        let id = buffers.create("t");
        buffers.get_mut(id).unwrap().insert(0, text).unwrap();
        let w = Window::new(&mut buffers, id);
        (buffers, w)
    }

    #[test]
    fn an_edit_before_point_moves_point() {
        let (mut buffers, w) = window_on("abcdef");
        w.set_point(&mut buffers, 3).unwrap();
        let b = buffers.get_mut(w.buffer()).unwrap();
        b.insert(1, "XY").unwrap();
        assert_eq!(w.point(&buffers), 5);
        buffers.get_mut(w.buffer()).unwrap().insert(5, "Z").unwrap();
        assert_eq!(w.point(&buffers), 5, "text inserted at point goes after it");
        buffers.get_mut(w.buffer()).unwrap().delete(1..6).unwrap();
        assert_eq!(w.point(&buffers), 1);
        assert_eq!(w.set_point(&mut buffers, 9), Err(Error::OutOfRange));
    }

    #[test]
    fn mark_is_unset_then_follows_edits() {
        let (mut buffers, mut w) = window_on("abcdef");
        assert_eq!(w.mark(&buffers), None);
        w.set_mark(&mut buffers, 4).unwrap();
        buffers.get_mut(w.buffer()).unwrap().delete(0..2).unwrap();
        assert_eq!(w.mark(&buffers), Some(2));
        w.set_mark(&mut buffers, 0).unwrap();
        assert_eq!(w.mark(&buffers), Some(0));
    }

    #[test]
    fn mark_ring_pushes_pops_and_drops_the_oldest() {
        let (mut buffers, mut w) = window_on("abcdefghijklmnopqrstuvwxyz");
        w.pop_mark();
        assert_eq!(w.mark(&buffers), None, "nothing to pop");
        w.push_mark(&mut buffers, 0, true).unwrap();
        let first = w.mark.unwrap();
        for p in 1..18 {
            w.push_mark(&mut buffers, p, p % 2 == 0).unwrap();
        }
        assert_eq!((w.mark(&buffers), w.active()), (Some(17), false));
        assert_eq!(w.ring(&buffers), (1..17).rev().collect::<Vec<_>>());
        w.set_active(true);
        w.pop_mark();
        assert_eq!((w.mark(&buffers), w.active()), (Some(16), false));
        assert_eq!(w.ring(&buffers)[..2], [15, 14]);
        assert_eq!(w.ring(&buffers).last(), Some(&17));
        let buffer = buffers.get(w.buffer()).unwrap();
        assert_eq!(
            buffer.marker(first),
            None,
            "the dropped mark's marker is removed"
        );
    }

    #[test]
    fn scrolling_follows_point() {
        // Every line is two chars, so line n starts at 2n.
        let (mut buffers, mut w) = window_on("0\n1\n2\n3\n4\n5\n6");
        assert_eq!(w.scroll(&buffers, 3), 0);
        for (line, top) in [(2, 0), (5, 3), (4, 3), (1, 1)] {
            w.set_point(&mut buffers, 2 * line).unwrap();
            assert_eq!(w.scroll(&buffers, 3), top, "point on line {line}");
        }
        assert_eq!(w.scroll(&buffers, 0), 1);
    }

    #[test]
    fn scrolling_counts_the_rows_of_wrapped_lines() {
        // 5 columns, so 4 chars to a row.
        let (mut buffers, mut w) = window_on(&"a".repeat(16));
        w.set_cols(5);
        w.set_point(&mut buffers, 16).unwrap();
        assert_eq!(
            (w.scroll(&buffers, 3), w.top_row()),
            (0, 1),
            "end of a 4-row line"
        );
        w.set_point(&mut buffers, 5).unwrap();
        assert_eq!(
            (w.scroll(&buffers, 3), w.top_row()),
            (0, 1),
            "row 1 is shown"
        );
        w.set_point(&mut buffers, 0).unwrap();
        assert_eq!(
            (w.scroll(&buffers, 3), w.top_row()),
            (0, 0),
            "back to row 0"
        );

        let (mut buffers, mut w) = window_on("aaaaaaaa\nbbbbbbbb\nc");
        w.set_cols(5);
        w.set_point(&mut buffers, 18).unwrap();
        assert_eq!(
            (w.scroll(&buffers, 3), w.top_row()),
            (1, 0),
            "below two 2-row lines"
        );
    }

    #[test]
    fn hscroll_follows_the_column_and_resets_when_it_fits() {
        let (mut buffers, mut w) = window_on("");
        let cols = 40;
        for (col, left) in [(39, 0), (100, 61), (80, 61), (50, 50), (60, 50), (39, 0)] {
            assert_eq!(w.hscroll(col, cols), left, "column {col}");
        }
        w.hscroll(100, cols);
        assert_eq!(w.split(&mut buffers).left(), 61, "a split keeps it");
        let other = buffers.create("o");
        w.show(&mut buffers, other);
        assert_eq!(w.left(), 0, "showing another buffer resets it");
        assert_eq!(w.hscroll(100, 0), 0, "no columns, no scroll");
    }

    #[test]
    fn a_split_copies_point_and_a_released_window_stops_following_edits() {
        let (mut buffers, mut w) = window_on("abcdef");
        w.set_point(&mut buffers, 3).unwrap();
        w.push_mark(&mut buffers, 1, true).unwrap();
        let new = w.split(&mut buffers);
        assert_eq!((new.point(&buffers), new.mark(&buffers)), (3, None));
        let (point, mark) = (w.point, w.mark.unwrap());
        w.release(&mut buffers);
        buffers
            .get_mut(w.buffer())
            .unwrap()
            .insert(0, "XY")
            .unwrap();
        let buffer = buffers.get(w.buffer()).unwrap();
        assert_eq!([point, mark].map(|m| buffer.marker(m)), [None; 2]);
        assert_eq!(
            new.point(&buffers),
            5,
            "the other window still follows edits"
        );
    }

    #[test]
    fn showing_another_buffer_drops_markers_from_the_old_one() {
        let (mut buffers, mut w) = window_on("abc");
        let old = w.buffer();
        w.set_point(&mut buffers, 2).unwrap();
        w.push_mark(&mut buffers, 1, true).unwrap();
        w.push_mark(&mut buffers, 0, true).unwrap();
        let (point, mark, ring) = (w.point, w.mark.unwrap(), w.ring[0]);
        let new = buffers.create("n");
        w.show(&mut buffers, new);
        assert_eq!(
            (w.buffer(), w.point(&buffers), w.mark(&buffers)),
            (new, 0, None)
        );
        assert_eq!((w.ring(&buffers), w.active()), (vec![], false));
        let old = buffers.get(old).unwrap();
        let markers = [point, mark, ring].map(|m| old.marker(m));
        assert_eq!(markers, [None; 3]);
    }
}
