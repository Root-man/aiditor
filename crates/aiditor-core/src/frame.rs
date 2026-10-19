//! Windows and the frame's display model.

use crate::buffer::Buffers;
use crate::settings::Settings;
use crate::window::{Tree, Window, WindowId};
use crate::{Core, Ctx, display};

impl Ctx<'_> {
    /// Splits the selected window in two, the new one below or right of it.
    pub(crate) fn split_window(&mut self, side_by_side: bool) -> Result<(), String> {
        self.outside_minibuffer("Attempt to split minibuffer window")?;
        let core = &mut *self.core;
        let id = WindowId(core.next_window);
        core.next_window += 1;
        let new = core.window.split(&mut core.buffers);
        core.others.insert(id, new);
        core.tree.split(core.selected, id, side_by_side);
        Ok(())
    }

    /// Selects the window `n` on from the selected one in tree order, wrapping round.
    pub(crate) fn other_window(&mut self, n: i64) -> Result<(), String> {
        // shortcut: Emacs leaves the minibuffer active in another window; allow it once commands can come back to it.
        self.outside_minibuffer("Can't leave the minibuffer")?;
        let core = &mut *self.core;
        let leaves = core.tree.leaves();
        let at = leaves.iter().position(|&id| id == core.selected);
        let at = at.expect("the selected window is in the tree") as i64;
        let next = (at + n.rem_euclid(leaves.len() as i64)) as usize % leaves.len();
        core.select(leaves[next]);
        Ok(())
    }

    /// Deletes the selected window, selecting the sibling that takes its space.
    pub(crate) fn delete_window(&mut self) -> Result<(), String> {
        let core = &mut *self.core;
        let old = core.selected;
        let next = (core.tree.remove(old))
            .ok_or("Attempt to delete minibuffer or sole ordinary window")?;
        core.select(next);
        core.others
            .remove(&old)
            .expect("deleted window is live")
            .release(&mut core.buffers);
        Ok(())
    }

    /// Deletes every window but the selected one.
    pub(crate) fn delete_other_windows(&mut self) -> Result<(), String> {
        self.outside_minibuffer("Can't expand minibuffer to full frame")?;
        let core = &mut *self.core;
        for (_, window) in core.others.drain() {
            window.release(&mut core.buffers);
        }
        core.tree = Tree::Leaf(core.selected);
        Ok(())
    }
}

impl Core {
    /// The selected window's display model for a window `height` px high, mode line included.
    /// It has no width, so point's line never scrolls sideways.
    #[cfg(test)]
    pub fn display(&mut self, height: f32) -> display::WindowDisplay {
        let (window, buffers, settings) = (&mut self.window, &self.buffers, &self.settings);
        window_display(window, buffers, settings, height, usize::MAX)
    }

    /// Every window's display model, laid out in the size last reported by `Msg::Resize`.
    pub fn frame(&mut self) -> display::FrameDisplay {
        let (width, height, cell_width) = self.frame_size;
        let cell = (cell_width, self.settings.line_height());
        let frame = display::Rect {
            x: 0.,
            y: 0.,
            width,
            height,
        };
        let windows = (self.tree.layout(frame, cell).into_iter())
            .map(|(id, rect)| {
                let selected = id == self.selected;
                let window = match selected {
                    true => &mut self.window,
                    false => self.others.get_mut(&id).expect("tree windows are live"),
                };
                let cols = (rect.width / cell_width) as usize;
                let (buffers, settings) = (&self.buffers, &self.settings);
                let display = window_display(window, buffers, settings, rect.height, cols);
                display::FrameWindow {
                    id,
                    rect,
                    selected,
                    display,
                }
            })
            .collect();
        display::FrameDisplay {
            windows,
            message: self.message.clone(),
            minibuffer: self.minibuffer(),
        }
    }

    /// Makes window `id` the selected one.
    pub(crate) fn select(&mut self, id: WindowId) {
        if id == self.selected {
            return;
        }
        let window = self.others.remove(&id).expect("selected window is live");
        let old = std::mem::replace(&mut self.window, window);
        self.others.insert(self.selected, old);
        self.selected = id;
    }

    /// The window's first line once point's line is among the `rows` it shows.
    #[cfg(test)]
    pub fn scroll(&mut self, rows: usize) -> usize {
        self.window.scroll(&self.buffers, rows)
    }
}

/// `window`'s display model for a window `height` px high and `cols` chars wide, mode line
/// included, point's line shown from the column that keeps point visible.
pub(crate) fn window_display(
    window: &mut Window,
    buffers: &Buffers,
    settings: &Settings,
    height: f32,
    cols: usize,
) -> display::WindowDisplay {
    let rows = ((height / settings.line_height()) as usize)
        .saturating_sub(1)
        .max(1);
    // shortcut: scrolls in buffer lines, so folds above point scroll it further than needed and a top line inside a fold repeats its placeholder; scroll in display lines when folds get common.
    let top = window.scroll(buffers, rows);
    let buffer = buffers.get(window.buffer()).expect("window buffer is live");
    let point = window.point(buffers);
    let mark = window.mark(buffers).filter(|_| window.active());
    // shortcut: point's column is its char column in the buffer line, so a fold before point on its line shifts the scroll; use the display column when folds get common.
    // shortcut: a tab or wide char counts as one column, so the cursor can sit past the edge on such lines; measure widths when the display does.
    let col = buffer.line_col(point).map_or(0, |(_, c)| c);
    let left = window.hscroll(col, cols);
    // One more than fits, for the partly visible last column; no width cuts nothing.
    let width = match cols {
        0 => usize::MAX,
        _ => cols.saturating_add(1),
    };
    let shown = left..left.saturating_add(width);
    display::build(buffer, top, rows, point, mark, settings, shown)
}
