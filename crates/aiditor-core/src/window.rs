//! Windows: viewports onto buffers, holding point and mark as markers and the first line shown.

use crate::buffer::{Buffer, BufferId, Buffers, Error, InsertionType, MarkerId};

pub struct Window {
    buffer: BufferId,
    point: MarkerId,
    mark: Option<MarkerId>,
    /// The first line shown; moves only to keep point's line visible.
    top: usize,
}

impl Window {
    /// A window on `buffer` with point at its start.
    pub fn new(buffers: &mut Buffers, buffer: BufferId) -> Self {
        Window {
            buffer,
            // Stay, like Emacs's window-point: typing commands move point themselves.
            point: buffers
                .get_mut(buffer)
                .expect("window buffer is live")
                .add_marker(0, InsertionType::Stay)
                .expect("0 is in every buffer"),
            mark: None,
            top: 0,
        }
    }

    pub fn buffer(&self) -> BufferId {
        self.buffer
    }

    /// Shows `buffer` instead, with point at its start; the old buffer keeps no markers of ours.
    pub fn show(&mut self, buffers: &mut Buffers, buffer: BufferId) {
        if let Some(old) = buffers.get_mut(self.buffer) {
            old.remove_marker(self.point);
            if let Some(m) = self.mark {
                old.remove_marker(m);
            }
        }
        *self = Window::new(buffers, buffer);
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

    /// Moves the first line just far enough that point's line is among the `rows` shown, and returns it.
    pub fn scroll(&mut self, buffers: &Buffers, rows: usize) -> usize {
        let buffer = buffers.get(self.buffer).expect("window buffer is live");
        let (line, _) = buffer.line_col(self.point(buffers)).unwrap_or_default();
        self.top = self.top.clamp(line.saturating_sub(rows.max(1) - 1), line);
        self.top
    }

    fn live<'a>(&self, buffers: &'a mut Buffers) -> Result<&'a mut Buffer, Error> {
        buffers
            .get_mut(self.buffer)
            .ok_or(Error::NoSuchBuffer(self.buffer))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn showing_another_buffer_drops_markers_from_the_old_one() {
        let (mut buffers, mut w) = window_on("abc");
        let old = w.buffer();
        w.set_point(&mut buffers, 2).unwrap();
        w.set_mark(&mut buffers, 1).unwrap();
        let (point, mark) = (w.point, w.mark.unwrap());
        let new = buffers.create("n");
        w.show(&mut buffers, new);
        assert_eq!(
            (w.buffer(), w.point(&buffers), w.mark(&buffers)),
            (new, 0, None)
        );
        let old = buffers.get(old).unwrap();
        assert_eq!((old.marker(point), old.marker(mark)), (None, None));
    }
}
