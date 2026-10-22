//! Windows and the frame's display model.

use crate::buffer::{BufferId, Buffers};
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

    /// Shows buffer `id` in a window other than the selected one, splitting a lone window, and
    /// keeps the selection, as Emacs's `display-buffer`; a window already showing it is enough.
    pub(crate) fn display_buffer(&mut self, id: BufferId) {
        let core = &mut *self.core;
        if std::iter::once(&core.window)
            .chain(core.others.values())
            .any(|w| w.buffer() == id)
        {
            return;
        }
        if core.tree.leaves().len() == 1 {
            self.split_window(false).expect("outside the minibuffer");
        }
        let core = &mut *self.core;
        let leaves = core.tree.leaves();
        let at = leaves.iter().position(|&w| w == core.selected);
        let next = leaves[(at.expect("the selected window is in the tree") + 1) % leaves.len()];
        let window = core.others.get_mut(&next).expect("a leaf is live");
        window.show(&mut core.buffers, id);
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
        let unwatched = self.projects.iter().any(|p| {
            p.project.name == self.current_workspace().project && !self.project_watched(&p.project)
        });
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
                let mut display = window_display(window, buffers, settings, rect.height, cols);
                if unwatched {
                    display.mode_line.push_str(" [unwatched]");
                }
                display::FrameWindow {
                    id,
                    rect,
                    selected,
                    display,
                }
            })
            .collect();
        let buffer = self.current();
        let workspace = self.current_workspace();
        let title = format!(
            "{}{} | {} | {}",
            buffer.name,
            if buffer.modified() { " *" } else { "" },
            workspace.name,
            workspace.project
        );
        display::FrameDisplay {
            windows,
            message: self.message.clone(),
            minibuffer: self.minibuffer(),
            hint: self.hint(),
            title,
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

#[cfg(test)]
mod tests {
    use crate::Msg;
    use crate::display::FrameDisplay;
    use crate::display::FrameWindow;
    use crate::harness::Harness;
    use crate::harness::ask;
    use crate::harness::line_start;
    use crate::harness::on;
    use crate::harness::twenty_lines;
    use crate::window::WindowId;

    /// Each window's (id, point), in tree order, and the selected id.
    fn windows(h: &Harness) -> (Vec<(usize, usize)>, usize) {
        let core = &h.core;
        let point = |id| match core.others.get(&id) {
            Some(w) => w.point(&core.buffers),
            None => core.point(),
        };
        let all = core.tree.leaves().into_iter().map(|id| (id.0, point(id)));
        (all.collect(), core.selected.0)
    }

    #[test]
    fn splitting_keeps_point_and_selects_the_old_window() {
        let mut h = on("abcdef");
        h.keys("C-f C-f C-f C-x 2");
        assert_eq!(windows(&h), (vec![(0, 3), (1, 3)], 0));
        assert_eq!(h.core.window.mark(&h.core.buffers), None);
        h.keys("C-SPC C-x 3");
        assert_eq!(
            h.core.others[&crate::WindowId(2)].mark(&h.core.buffers),
            None,
            "no mark"
        );
    }

    #[test]
    fn two_windows_on_one_buffer_keep_separate_points() {
        let mut h = on("abcdef");
        h.keys("C-f C-f C-f C-x 2 M->");
        assert_eq!(windows(&h), (vec![(0, 6), (1, 3)], 0));
        h.keys("C-x o");
        assert_eq!((h.point(), h.core.selected.0), (3, 1));
        h.keys("C-x o");
        assert_eq!(h.point(), 6);
        h.keys("C-b C-b C-b C-b C-b X Y");
        assert_eq!(
            windows(&h),
            (vec![(0, 3), (1, 5)], 0),
            "the other point follows the edit"
        );
    }

    #[test]
    fn other_window_cycles_both_ways() {
        let mut h = on("abc");
        h.keys("C-x 2 C-x 2");
        let order: Vec<_> = windows(&h).0.iter().map(|w| w.0).collect();
        assert_eq!(order, [0, 2, 1]);
        h.keys("C-x o");
        assert_eq!(h.core.selected.0, 2);
        h.keys("C-x o C-x o");
        assert_eq!(h.core.selected.0, 0, "wraps to the first");
        h.keys("C-u - 1 C-x o");
        assert_eq!(h.core.selected.0, 1, "backward to the last");
        h.keys("C-u 4 C-x o");
        assert_eq!(h.core.selected.0, 0);
        h.keys("C-x 1 C-x o");
        assert_eq!(h.core.selected.0, 0, "one window does nothing");
    }

    #[test]
    fn deleting_windows() {
        let mut h = on("abcdef");
        h.keys("C-x 0");
        assert_eq!(
            (windows(&h), h.core.message()),
            (
                (vec![(0, 0)], 0),
                Some("Attempt to delete minibuffer or sole ordinary window")
            )
        );
        h.keys("C-f C-x 2 C-x o C-f C-f");
        h.keys("C-x 0");
        assert_eq!(
            windows(&h),
            (vec![(0, 1)], 0),
            "the upper window with its point"
        );
        h.keys("C-x 2 C-x 2 C-x o C-x 1");
        assert_eq!(windows(&h), (vec![(3, 1)], 3), "only the second is left");
        assert_eq!(h.frame(80., 100.).windows[0].rect.height, 100.);
    }

    #[test]
    fn visiting_a_file_in_a_split() {
        let mut h = Harness::new(&[("a.txt", "abc")]);
        h.keys("x C-x 2");
        h.open("a.txt");
        let names: Vec<_> = (h.frame(80., 100.).windows.iter())
            .map(|w| w.display.mode_line.clone())
            .collect();
        assert_eq!(
            names,
            [
                "-- a.txt  1:0 [unwatched]",
                "** *scratch*<main>  1:1 [unwatched]"
            ]
        );
    }

    #[test]
    fn unwatched_status_follows_the_workspace_project_and_watch_setting() {
        let mut h = Harness::new(&[("other/x", "")]);
        h.core.projects[0].project.primary.path = h.path("");
        let lines = |h: &mut Harness| {
            h.frame(800., 600.)
                .windows
                .into_iter()
                .map(|w| w.display.mode_line)
                .collect::<Vec<_>>()
        };
        assert_eq!(lines(&mut h), ["-- *scratch*<main>  1:0 [unwatched]"]);
        h.keys("C-x p w home RET C-p RET");
        assert_eq!(lines(&mut h), ["-- *scratch*<main>  1:0"]);
        h.keys("C-x p w home RET C-n RET M-x");
        assert_eq!(lines(&mut h), ["-- *scratch*<main>  1:0 [unwatched]"]);
        h.keys("C-g");
        h.core
            .projects
            .push(crate::project::Loaded::ad_hoc("other", &h.path("other")));
        h.core.new_workspace("other", "other");
        h.send(crate::Msg::Tick);
        assert_eq!(lines(&mut h), ["-- *scratch*<other>  1:0"]);
        h.keys("C-x p p home RET");
        assert_eq!(lines(&mut h), ["-- *scratch*<main>  1:0 [unwatched]"]);
    }

    #[test]
    fn the_title_names_the_buffer_workspace_and_project() {
        let mut h = Harness::new(&[("t.txt", "")]);
        h.open("t.txt");
        let main = h.core.workspace();
        assert_eq!(h.frame(80., 100.).title, "t.txt | main | home");
        h.keys("x");
        assert_eq!(h.frame(80., 100.).title, "t.txt * | main | home");
        h.core.new_workspace("w2", "other");
        assert_eq!(h.frame(80., 100.).title, "*scratch*<w2> | w2 | other");
        h.core.rename_workspace("w3").unwrap();
        assert_eq!(h.frame(80., 100.).title, "*scratch*<w3> | w3 | other");
        h.core.switch_workspace(main).unwrap();
        assert_eq!(h.frame(80., 100.).title, "t.txt * | main | home");
    }

    #[test]
    fn frame_display_of_a_stacked_split() {
        let mut h = on("one\ntwo");
        h.keys("C-n C-x 2");
        insta::assert_debug_snapshot!(h.frame(800., 600.));
    }

    #[test]
    fn frame_display_of_a_side_by_side_split() {
        let mut h = on("one\ntwo");
        h.keys("C-x 3 C-n");
        insta::assert_debug_snapshot!(h.frame(800., 600.));
    }

    /// Each shown line's text and the cursor of a frame's first window.
    fn shown(frame: &FrameDisplay) -> (Vec<String>, Option<(usize, usize)>) {
        let d = &frame.windows[0].display;
        let text = |l: &crate::display::Line| l.spans.iter().map(|s| s.text.as_str()).collect();
        (d.lines.iter().map(text).collect(), d.cursor)
    }

    #[test]
    fn point_s_line_scrolls_sideways_to_keep_point_visible() {
        let long: String = (0..100).map(|i| char::from(b'a' + i % 26)).collect();
        let mut h = on(&format!("{long}\n{long}\n{long}"));
        h.keys("C-n");
        // 40 columns of 8 px cells.
        let (width, height) = (320., 600.);
        h.keys("C-e");
        let (lines, cursor) = shown(&h.frame(width, height));
        // Each line holds 40 columns plus the partly visible 41st.
        assert_eq!(lines, [&long[..41], &long[61..], &long[..41]]);
        assert_eq!(cursor, Some((1, 39)));
        h.keys("C-a");
        let (lines, cursor) = shown(&h.frame(width, height));
        let unscrolled = long[..41].to_string();
        assert_eq!((lines, cursor), (vec![unscrolled; 3], Some((1, 0))));
    }

    #[test]
    fn a_line_far_wider_than_the_window_holds_only_the_shown_columns() {
        let long: String = (0..10_000)
            .map(|i| char::from(b'a' + (i % 26) as u8))
            .collect();
        let mut h = on(&format!("{long}\nnext"));
        let id = h.core.window.buffer();
        let buffer = h.core.buffers.get_mut(id).unwrap();
        let rule = crate::buffer::Prop::Line(crate::buffer::Decoration::Rule(0x888888));
        buffer.add_property(9_990..9_995, rule).unwrap();
        // Before any resize the frame has no size: one row and no width.
        let unsized_frame = h.core.frame();
        assert_eq!(
            shown(&unsized_frame).0[0].len(),
            10_000,
            "no width cuts nothing"
        );
        // 20 columns of 8 px cells.
        h.keys("C-e");
        let frame = h.frame(160., 600.);
        assert_eq!(
            shown(&frame),
            (vec![long[9_981..].into(), "next".into()], Some((0, 19)))
        );
        let decorations = &frame.windows[0].display.lines[0].decorations;
        assert_eq!(decorations.len(), 1, "a decoration in the cut-off tail");
        h.keys("C-n");
        let frame = h.frame(160., 600.);
        assert_eq!(shown(&frame).0, [&long[..21], "next"]);
        assert_eq!(frame.windows[0].display.lines[0].decorations.len(), 1);
        // A fold over the long line's newline still joins it to the next line.
        let buffer = h.core.buffers.get_mut(id).unwrap();
        let fold = crate::buffer::Prop::Invisible("…".into());
        buffer.add_property(9_998..10_002, fold).unwrap();
        let frame = h.frame(160., 600.);
        let lines = &frame.windows[0].display.lines;
        assert_eq!(shown(&frame).0, [&long[..21]]);
        assert_eq!((lines[0].number, lines.len()), (0, 1));
    }

    #[test]
    fn the_minibuffer_line_scrolls_sideways() {
        let mut h = on("text");
        ask(&mut h, &["Text: "]);
        // 20 columns, 14 after the prompt.
        h.frame(160., 600.);
        h.keys("C-c p abcdefghijklmnopqrstuvwxyzABCD");
        let mb = h.core.minibuffer().unwrap();
        let text: String = mb.display.lines[0].spans.iter().map(|s| &*s.text).collect();
        assert_eq!(
            (text.as_str(), mb.display.cursor),
            ("rstuvwxyzABCD", Some((0, 13)))
        );
    }

    #[test]
    fn a_click_on_the_scrolled_line_lands_on_the_clicked_char() {
        let long: String = (0..100).map(|i| char::from(b'a' + i % 26)).collect();
        let mut h = on(&format!("{long}\n{long}"));
        h.keys("C-e");
        h.frame(320., 600.);
        let click = |line| Msg::Click {
            window: WindowId(0),
            line,
            col: 0,
        };
        h.send(click(0));
        assert_eq!(h.point(), 61);
        h.send(click(1));
        assert_eq!(h.point(), 101, "another line is not scrolled");
    }

    #[test]
    fn clicks_select_and_scrolls_keep_the_selected_window() {
        let mut h = twenty_lines();
        h.keys("C-x 3");
        let right = h.frame(800., 600.).windows[1].id;
        h.send(Msg::Scroll {
            window: right,
            lines: 5,
        });
        assert_eq!(windows(&h), (vec![(0, 0), (1, line_start(&h, 5))], 0));
        h.send(Msg::Click {
            window: right,
            line: 7,
            col: 1,
        });
        assert_eq!(windows(&h), (vec![(0, 0), (1, line_start(&h, 7) + 1)], 1));
        h.keys("C-x 0").send(Msg::Click {
            window: right,
            line: 0,
            col: 0,
        });
        assert_eq!(
            windows(&h),
            (vec![(0, 0)], 0),
            "a deleted window's click is dropped"
        );
    }

    #[test]
    fn resizing_re_lays_out_snapped_to_lines() {
        let mut h = on("one");
        h.keys("C-x 2");
        let line = h.core.settings().line_height();
        // Heights in lines, to two places.
        let heights = |h: &mut Harness, lines: f32| -> Vec<f32> {
            let frame = h.frame(800., line * lines);
            let lines = |w: &FrameWindow| (w.rect.height / line * 100.).round() / 100.;
            frame.windows.iter().map(lines).collect()
        };
        assert_eq!(heights(&mut h, 10.), [5., 5.]);
        assert_eq!(heights(&mut h, 7.2), [4., 3.2]);
    }
}
