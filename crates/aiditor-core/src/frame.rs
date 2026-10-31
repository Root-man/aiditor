//! Windows and the frame's display model.

use crate::buffer::{BufferId, Buffers};
use crate::display::TEXT_INSET;
use crate::settings::{LineNumbers, Settings};
use crate::window::{Tree, Window, WindowId};
use crate::{Core, Ctx, display};

impl Ctx<'_> {
    /// Splits the selected window in two, the new one below or right of it.
    pub(crate) fn split_window(&mut self, side_by_side: bool) -> Result<(), String> {
        self.outside_minibuffer("Attempt to split minibuffer window")?;
        let core = &mut *self.core;
        let id = WindowId(core.next_window);
        core.next_window += 1;
        let new = core.windows.window.split(&mut core.buffers);
        core.windows.others.insert(id, new);
        core.windows
            .tree
            .split(core.windows.selected, id, side_by_side);
        Ok(())
    }

    /// Shows buffer `id` in a window other than the selected one, splitting a lone window, and
    /// keeps the selection, as Emacs's `display-buffer`; a window already showing it is enough.
    pub(crate) fn display_buffer(&mut self, id: BufferId) {
        let core = &mut *self.core;
        if std::iter::once(&core.windows.window)
            .chain(core.windows.others.values())
            .any(|w| w.buffer() == id)
        {
            return;
        }
        if core.windows.tree.leaves().len() == 1 {
            self.split_window(false).expect("outside the minibuffer");
        }
        let core = &mut *self.core;
        let leaves = core.windows.tree.leaves();
        let at = leaves.iter().position(|&w| w == core.windows.selected);
        let next = leaves[(at.expect("the selected window is in the tree") + 1) % leaves.len()];
        let window = core.windows.others.get_mut(&next).expect("a leaf is live");
        window.show(&mut core.buffers, id);
    }

    /// Selects the window `n` on from the selected one in tree order, wrapping round.
    pub(crate) fn other_window(&mut self, n: i64) -> Result<(), String> {
        // shortcut: Emacs leaves the minibuffer active in another window; allow it once commands can come back to it.
        self.outside_minibuffer("Can't leave the minibuffer")?;
        let core = &mut *self.core;
        let leaves = core.windows.tree.leaves();
        let at = leaves.iter().position(|&id| id == core.windows.selected);
        let at = at.expect("the selected window is in the tree") as i64;
        let next = (at + n.rem_euclid(leaves.len() as i64)) as usize % leaves.len();
        core.select(leaves[next]);
        Ok(())
    }

    /// Deletes the selected window, selecting the sibling that takes its space.
    pub(crate) fn delete_window(&mut self) -> Result<(), String> {
        let core = &mut *self.core;
        let old = core.windows.selected;
        let next = (core.windows.tree.remove(old))
            .ok_or("Attempt to delete minibuffer or sole ordinary window")?;
        core.select(next);
        core.windows
            .others
            .remove(&old)
            .expect("deleted window is live")
            .release(&mut core.buffers);
        Ok(())
    }

    /// Deletes every window but the selected one.
    pub(crate) fn delete_other_windows(&mut self) -> Result<(), String> {
        self.outside_minibuffer("Can't expand minibuffer to full frame")?;
        let core = &mut *self.core;
        for (_, window) in core.windows.others.drain() {
            window.release(&mut core.buffers);
        }
        core.windows.tree = Tree::Leaf(core.windows.selected);
        Ok(())
    }
}

impl Core {
    /// The selected window's display model for a window of `rows` text rows.
    /// It has no width, so point's line never scrolls sideways.
    #[cfg(test)]
    pub fn display(&mut self, rows: usize) -> display::WindowDisplay {
        let window = &mut self.windows.window;
        let (buffers, settings) = (&mut self.buffers, &self.settings);
        window_display(window, buffers, settings, rows, rows, usize::MAX)
    }

    /// Every window's display model, laid out in the size last reported by `Msg::Resize`.
    pub fn frame(&mut self) -> display::FrameDisplay {
        self.render_streams();
        let unwatched = self
            .projects
            .iter()
            .any(|p| p.id == self.current_workspace().project && !self.project_watched(p));
        let (width, height, cell_width) = self.frame_size;
        let cell = (cell_width, self.settings.line_height());
        let frame = display::Rect {
            x: 0.,
            y: 0.,
            width,
            height,
        };
        let chrome = self.settings.mode_line_height();
        let origin = self.prompts.minibuffer.as_ref().map(|mb| mb.from);
        let windows = (self.windows.tree.layout(frame, cell, chrome).into_iter())
            .map(|(id, rect)| {
                let selected = id == self.windows.selected;
                let window = match selected {
                    true => &mut self.windows.window,
                    false => self
                        .windows
                        .others
                        .get_mut(&id)
                        .expect("tree windows are live"),
                };
                let (buffers, settings) = (&mut self.buffers, &self.settings);
                let buffer = buffers.get(window.buffer()).expect("window buffer is live");
                let style = buffer.line_numbers(settings.line_numbers);
                // The window a minibuffer was opened from still shows where point is, as swiper's preview.
                let highlighted = settings.hl_line
                    && (selected || origin == Some(id))
                    && (buffer.numbered() || buffer.copy_mode());
                let width = display::gutter_width(buffer.len_lines(), cell_width);
                // A window too narrow for it keeps its one column of text.
                let room = rect.width - TEXT_INSET;
                let numbered = style != LineNumbers::Off && room - width > cell_width;
                let cols =
                    ((room - if numbered { width } else { TEXT_INSET }) / cell_width) as usize;
                let rows = settings.text_rows(rect.height);
                let drawn = settings.drawn_rows(rect.height);
                let mut display = window_display(window, buffers, settings, rows, drawn, cols);
                if highlighted {
                    display::highlight_line(&mut display);
                }
                let gutter = numbered.then(|| {
                    let buffer = buffers.get(window.buffer()).expect("window buffer is live");
                    let (point, _) = buffer.line_col(window.point(buffers)).unwrap_or_default();
                    let continued = window.top_row() > 0;
                    let lines = &display.lines;
                    display::gutter(lines, style, width, point, continued, &settings.theme)
                });
                if unwatched
                    && buffers
                        .get(window.buffer())
                        .is_some_and(|buffer| buffer.file.is_some())
                {
                    display.mode_line.push_str(" [unwatched]");
                }
                display::FrameWindow {
                    id,
                    rect,
                    selected,
                    display,
                    gutter,
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
            self.project_name(workspace.project)
        );
        let (project, workspace) = fit_names(
            self.project_name(workspace.project),
            &workspace.name,
            // A monospace advance is linear in the font size.
            ((width - 2. * TEXT_INSET) * self.settings.font_size
                / (cell_width * self.settings.workspace_font_size())) as usize,
        );
        display::FrameDisplay {
            windows,
            message: self.message.clone(),
            minibuffer: self.minibuffer(),
            hint: self.hint(),
            title,
            project,
            workspace,
        }
    }

    /// Makes window `id` the selected one.
    pub(crate) fn select(&mut self, id: WindowId) {
        if id == self.windows.selected {
            return;
        }
        self.pending_visit = None;
        let window = self
            .windows
            .others
            .remove(&id)
            .expect("selected window is live");
        let old = std::mem::replace(&mut self.windows.window, window);
        self.windows.others.insert(self.windows.selected, old);
        self.windows.selected = id;
    }

    /// The window's first line once point's line is among the `rows` it shows.
    #[cfg(test)]
    pub fn scroll(&mut self, rows: usize) -> usize {
        self.windows.window.scroll(&self.buffers, rows)
    }
}

/// `window`'s display model `cols` chars wide, keeping point in the first `rows` of the `drawn`
/// text rows, point's line shown from the column that keeps point visible.
pub(crate) fn window_display(
    window: &mut Window,
    buffers: &mut Buffers,
    settings: &Settings,
    rows: usize,
    drawn: usize,
    cols: usize,
) -> display::WindowDisplay {
    window.set_cols(cols);
    // shortcut: scrolls in buffer lines, so folds above point scroll it further than needed and a top line inside a fold repeats its placeholder; scroll in display lines when folds get common.
    let top = window.scroll(buffers, rows);
    let id = window.buffer();
    buffers
        .get_mut(id)
        .expect("window buffer is live")
        .reparse();
    let buffers = &*buffers;
    let buffer = buffers.get(id).expect("window buffer is live");
    let point = window.point(buffers);
    let mark = window.mark(buffers).filter(|_| window.active());
    // shortcut: point's column is its char column in the buffer line, so a fold before point on its line shifts the scroll; use the display column when folds get common.
    // shortcut: a tab or wide char counts as one column, so the cursor can sit past the edge on such lines; measure widths when the display does.
    let wrap = (buffer.wrap(cols)).map(|(width, word)| display::Wrap {
        width,
        word,
        from: window.row_starts(buffer, top)[window.top_row()],
    });
    // A wrapped line never scrolls sideways.
    let col = match wrap {
        Some(_) => 0,
        None => buffer.line_col(point).map_or(0, |(_, c)| c),
    };
    let left = window.hscroll(col, cols);
    // One more than fits, for the partly visible last column; no width cuts nothing.
    let width = match cols {
        0 => usize::MAX,
        _ => cols.saturating_add(1),
    };
    // Wrapped, a line is laid out only from the window's first row through the rows it can show.
    let shown = match wrap {
        Some(_) => 0..(drawn + 1).saturating_mul(cols),
        None => left..left.saturating_add(width),
    };
    let mut display = display::build(buffer, top, drawn, point, mark, settings, shown, wrap);
    if let Some(terminal) = buffer.terminal().filter(|_| !buffer.copy_mode()) {
        (display.cursor, display.terminal) = (None, Some(terminal.display(&settings.theme)));
    }
    display
}

/// Cuts the workspace modeline's names so `<project> › <workspace>` fits in `cols`: a name
/// that fits in half the room stays whole and the other takes the rest, else each takes half.
fn fit_names(project: &str, workspace: &str, cols: usize) -> (String, String) {
    let (p, w) = (project.chars().count(), workspace.chars().count());
    let room = cols.saturating_sub(" › ".chars().count());
    if p + w <= room {
        return (project.to_string(), workspace.to_string());
    }
    let half = room / 2;
    let (p_room, w_room) = if p <= half {
        (p, room - p)
    } else if w <= room - half {
        (room - w, w)
    } else {
        (half, room - half)
    };
    let cut = |name: &str, len: usize, keep: usize| match keep >= len {
        true => name.to_string(),
        false => (name.chars().take(keep.saturating_sub(1)))
            .chain((keep > 0).then_some('…'))
            .collect(),
    };
    (cut(project, p, p_room), cut(workspace, w, w_room))
}

#[cfg(test)]
mod tests {
    use crate::Msg;
    use crate::buffer::Decoration;
    use crate::display::FrameDisplay;
    use crate::display::FrameWindow;
    use crate::harness::Harness;
    use crate::harness::ask;
    use crate::harness::on;
    use crate::settings::{FaceName, Settings};

    /// Each window's (id, point), in tree order, and the selected id.
    fn windows(h: &Harness) -> (Vec<(usize, usize)>, usize) {
        let core = &h.core;
        let point = |id| match core.windows.others.get(&id) {
            Some(w) => w.point(&core.buffers),
            None => core.point(),
        };
        let all = core
            .windows
            .tree
            .leaves()
            .into_iter()
            .map(|id| (id.0, point(id)));
        (all.collect(), core.windows.selected.0)
    }

    #[test]
    fn splitting_keeps_point_and_selects_the_old_window() {
        let mut h = on("abcdef");
        h.keys("C-f C-f C-f C-x 2");
        assert_eq!(windows(&h), (vec![(0, 3), (1, 3)], 0));
        assert_eq!(h.core.windows.window.mark(&h.core.buffers), None);
        h.keys("C-SPC C-x 3");
        assert_eq!(
            h.core.windows.others[&crate::WindowId(2)].mark(&h.core.buffers),
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
        assert_eq!((h.point(), h.core.windows.selected.0), (3, 1));
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
        assert_eq!(h.core.windows.selected.0, 2);
        h.keys("C-x o C-x o");
        assert_eq!(h.core.windows.selected.0, 0, "wraps to the first");
        h.keys("C-u - 1 C-x o");
        assert_eq!(h.core.windows.selected.0, 1, "backward to the last");
        h.keys("C-u 4 C-x o");
        assert_eq!(h.core.windows.selected.0, 0);
        h.keys("C-x 1 C-x o");
        assert_eq!(h.core.windows.selected.0, 0, "one window does nothing");
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
        for _ in 0..2 {
            let names: Vec<_> = (h.frame(80., 100.).windows.iter())
                .map(|w| w.display.mode_line.clone())
                .collect();
            assert_eq!(
                names,
                ["-- a.txt  1:0 [unwatched]", "** *scratch*<main>  1:1"]
            );
            h.keys("C-x o");
        }
    }

    #[test]
    fn fileless_buffers_show_ordinary_mode_lines_in_unwatched_projects() {
        let mut h = Harness::new(&[]);
        assert_eq!(
            h.frame(800., 600.).windows[0].display.mode_line,
            "-- *scratch*<main>  1:0"
        );
        h.keys("C-x t l M-<");
        assert_eq!(
            h.frame(800., 600.).windows[0].display.mode_line,
            "%% *Workspaces*  1:0"
        );
        h.keys("q M-z C-x b *Messages* RET M-<");
        assert_eq!(
            h.frame(800., 600.).windows[0].display.mode_line,
            "%% *Messages*  1:0"
        );
    }

    #[test]
    fn unwatched_status_follows_the_workspace_project_and_watch_setting() {
        let mut h = Harness::new(&[("other/x", "")]);
        h.index_reply("other", &["x"], &["other"]);
        h.index_reply("", &["other/x"], &["", "other"]);
        h.core.projects[0].project.primary.path = h.path("");
        h.open("other/x");
        let lines = |h: &mut Harness| {
            h.frame(800., 600.)
                .windows
                .into_iter()
                .map(|w| w.display.mode_line)
                .collect::<Vec<_>>()
        };
        assert_eq!(lines(&mut h), ["-- x  1:0 [unwatched]"]);
        h.keys("C-x p w home RET C-p RET");
        assert_eq!(lines(&mut h), ["-- x  1:0"]);
        h.keys("C-x p w home RET C-n RET M-x");
        assert_eq!(lines(&mut h), ["-- x  1:0 [unwatched]"]);
        h.keys("C-g");
        h.core
            .add_project(crate::project::Loaded::ad_hoc("other", &h.path("other")));
        h.new_workspace("other", "other");
        h.open("other/x");
        h.send(crate::Msg::Tick);
        assert_eq!(lines(&mut h), ["-- x<2>  1:0"]);
        h.keys("C-x p p home RET");
        assert_eq!(lines(&mut h), ["-- x  1:0 [unwatched]"]);
    }

    #[test]
    fn the_title_names_the_buffer_workspace_and_project() {
        let mut h = Harness::new(&[("t.txt", "")]);
        h.open("t.txt");
        let main = h.core.workspace();
        assert_eq!(h.frame(80., 100.).title, "t.txt | main | home");
        h.keys("x");
        assert_eq!(h.frame(80., 100.).title, "t.txt * | main | home");
        h.new_workspace("w2", "other");
        assert_eq!(h.frame(80., 100.).title, "*scratch*<w2> | w2 | other");
        h.core.rename_workspace("w3").unwrap();
        assert_eq!(h.frame(80., 100.).title, "*scratch*<w3> | w3 | other");
        h.core.switch_workspace(main).unwrap();
        assert_eq!(h.frame(80., 100.).title, "t.txt * | main | home");
    }

    #[test]
    fn the_workspace_modeline_names_the_current_project_and_workspace() {
        let mut h = Harness::new(&[("t.txt", "")]);
        let names = |h: &mut Harness| {
            let frame = h.frame(800., 600.);
            (frame.project, frame.workspace)
        };
        let main = h.core.workspace();
        let home = || ("home".to_string(), "main".to_string());
        assert_eq!(names(&mut h), home());
        h.keys("C-x 3 C-x o");
        h.open("t.txt");
        assert_eq!(names(&mut h), home());
        h.keys("C-x C-f");
        assert!(h.core.minibuffer().is_some());
        assert_eq!(names(&mut h), home());
        h.keys("C-g");
        h.new_workspace("w2", "other");
        h.core.rename_workspace("w3").unwrap();
        assert_eq!(names(&mut h), ("other".into(), "w3".into()));
        h.core.switch_workspace(main).unwrap();
        assert_eq!(names(&mut h), home());
    }

    #[test]
    fn the_workspace_modeline_names_are_cut_to_the_frame_width() {
        let fit = |p: &str, w: &str, cols| super::fit_names(p, w, cols);
        let s = |p: &str, w: &str| (p.to_string(), w.to_string());
        assert_eq!(
            fit("aiditor", "watcher-fix", 40),
            s("aiditor", "watcher-fix")
        );
        assert_eq!(
            fit("aiditor", "watcher-fix", 21),
            s("aiditor", "watcher-fix")
        );
        assert_eq!(
            fit("aiditor", "watcher-fix", 20),
            s("aiditor", "watcher-f…")
        );
        let long = "x".repeat(60);
        let (p, w) = fit("Legacy crap", &long, 40);
        assert_eq!((p.as_str(), w.chars().count()), ("Legacy crap", 26));
        assert!(w.ends_with('…'));
        let (p, w) = fit(&long, "short", 40);
        assert_eq!((p.chars().count(), w.as_str()), (32, "short"));
        let (p, w) = fit(&"p".repeat(30), &"w".repeat(30), 23);
        assert_eq!(
            (p, w),
            (format!("{}…", "p".repeat(9)), format!("{}…", "w".repeat(9)))
        );
        assert_eq!(fit("abc", "def", 5), s("…", "…"));
        assert_eq!(fit("abc", "def", 2), s("", ""));
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
        h.keys("M-x toggle-truncate-lines RET C-n");
        // 40 columns of 8 px cells.
        let (width, height) = (344., 600.);
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
        let id = h.core.windows.window.buffer();
        let buffer = h.core.buffers.get_mut(id).unwrap();
        buffer.truncate_lines = true;
        let rule = crate::buffer::Prop::Line(crate::buffer::Decoration::Rule(
            crate::settings::FaceName::SessionThought,
        ));
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
        let frame = h.frame(184., 600.);
        assert_eq!(
            shown(&frame),
            (vec![long[9_981..].into(), "next".into()], Some((0, 19)))
        );
        let decorations = &frame.windows[0].display.lines[0].decorations;
        assert_eq!(
            decorations.len(),
            2,
            "hl-line and a decoration in the cut-off tail"
        );
        h.keys("C-n");
        let frame = h.frame(184., 600.);
        assert_eq!(shown(&frame).0, [&long[..21], "next"]);
        assert_eq!(frame.windows[0].display.lines[0].decorations.len(), 1);
        // A fold over the long line's newline still joins it to the next line.
        let buffer = h.core.buffers.get_mut(id).unwrap();
        let fold = crate::buffer::Prop::Invisible("…".into());
        buffer.add_property(9_998..10_002, fold).unwrap();
        let frame = h.frame(184., 600.);
        let lines = &frame.windows[0].display.lines;
        assert_eq!(shown(&frame).0, [&long[..21]]);
        assert_eq!((lines[0].number, lines.len()), (0, 1));
    }

    #[test]
    fn long_lines_wrap_at_the_edge_in_code_and_at_words_in_prose() {
        let mut h = Harness::new(&[
            ("a.rs", "abcdefghij\nabcd"),
            ("a.md", "one two three"),
            ("b.md", "abcdefghij"),
        ]);
        // 5 columns of 8 px cells, 4 chars to a row.
        h.open("a.rs");
        let frame = h.frame(64., 600.);
        assert_eq!(
            shown(&frame).0,
            ["abcd\\", "efgh\\", "ij", "abcd"],
            "a line that fits has no marker"
        );
        let lines = &frame.windows[0].display.lines;
        let numbers: Vec<_> = lines.iter().map(|l| l.number).collect();
        assert_eq!(numbers, [0, 0, 0, 1]);
        let comment = h.core.settings().theme.syntax["comment"].fg;
        let marker = lines[0].spans.last().unwrap();
        assert_eq!((marker.text.as_str(), marker.face.fg), ("\\", comment));
        h.keys("C-f C-f C-f C-f C-f");
        assert_eq!(
            shown(&h.frame(64., 600.)).1,
            Some((1, 1)),
            "the cursor on row 1"
        );
        h.keys("C-f C-f C-f");
        assert_eq!(
            shown(&h.frame(64., 600.)).1,
            Some((2, 0)),
            "point at a break"
        );
        // 9 columns, 8 chars to a row.
        h.open("a.md");
        assert_eq!(shown(&h.frame(96., 600.)).0, ["one two ", "three"]);
        h.open("b.md");
        assert_eq!(
            shown(&h.frame(64., 600.)).0,
            ["abcd", "efgh", "ij"],
            "a long word"
        );
    }

    #[test]
    fn a_long_wrapped_line_is_laid_out_only_through_the_shown_rows() {
        let long: String = (0..10_000)
            .map(|i| char::from(b'a' + (i % 26) as u8))
            .collect();
        let mut h = Harness::new(&[("a.rs", &long)]);
        h.open("a.rs").keys("M->");
        let height = 2. * h.core.settings().line_height() + h.core.settings().mode_line_height();
        // 21 columns, 20 chars to a row; 500 rows, the last one holding point.
        let (lines, cursor) = shown(&h.frame(192., height));
        let rows = [format!("{}\\", &long[9_960..9_980]), long[9_980..].into()];
        assert_eq!((lines, cursor), (rows.to_vec(), Some((1, 20))));
        assert_eq!(h.core.windows.window.top_row(), 498);
    }

    #[test]
    fn a_window_deep_inside_a_word_wrapped_line_breaks_as_the_whole_line() {
        let long = "word ".repeat(1_000);
        let mut h = Harness::new(&[("a.md", &long)]);
        h.open("a.md").keys("M->");
        let height = 3. * h.core.settings().line_height() + h.core.settings().mode_line_height();
        // 21 columns, 20 chars to a row, 3 rows.
        let (lines, cursor) = shown(&h.frame(192., height));
        let starts = crate::display::row_starts(long.len(), 20, true, |i| &long[i..=i] == " ");
        let ends = starts[1..].iter().copied().chain([long.len()]);
        let rows: Vec<String> = (starts.iter().zip(ends))
            .map(|(&s, e)| long[s..e].into())
            .collect();
        let last = rows[rows.len() - 3..].to_vec();
        let cursor_col = last[2].len();
        assert_eq!((lines, cursor), (last, Some((2, cursor_col))));
    }

    #[test]
    fn a_fold_on_a_wrapped_line_is_split_with_it() {
        let mut h = Harness::new(&[("a.rs", "abcdefghij")]);
        h.open("a.rs");
        let id = h.core.windows.window.buffer();
        let fold = crate::buffer::Prop::Invisible("…".into());
        (h.core.buffers.get_mut(id).unwrap().add_property(1..3, fold)).unwrap();
        let frame = h.frame(64., 600.);
        assert_eq!(shown(&frame).0, ["a…de\\", "fghi\\", "j"]);
        assert!(frame.windows[0].display.lines[0].spans[1].fold);
    }

    #[test]
    fn toggle_truncate_lines_switches_between_wrapping_and_truncation() {
        let mut h = on("abcdefghij");
        h.keys("M-x toggle-truncate-lines RET");
        let frame = h.frame(64., 600.);
        assert_eq!(
            frame.message.as_deref(),
            Some("Truncate long lines enabled")
        );
        assert_eq!(shown(&frame).0, ["abcdef"], "cut at the edge");
        h.keys("M-x toggle-truncate-lines RET");
        let frame = h.frame(64., 600.);
        assert_eq!(
            frame.message.as_deref(),
            Some("Truncate long lines disabled")
        );
        assert_eq!(
            shown(&frame).0,
            ["abcd", "efgh", "ij"],
            "a text file wraps at words"
        );
    }

    #[test]
    fn a_window_inside_a_wrapped_line_shows_from_its_row() {
        // 4 rows of 4 chars, as a text file, in a window of 2 rows and the mode line.
        let mut h = on(&"abcd".repeat(4));
        let height = 2. * h.core.settings().line_height() + h.core.settings().mode_line_height();
        h.keys("M->");
        let frame = h.frame(64., height);
        assert_eq!(
            shown(&frame),
            (vec!["abcd".into(), "abcd".into()], Some((1, 4)))
        );
    }

    #[test]
    fn the_minibuffer_line_scrolls_sideways() {
        let mut h = on("text");
        ask(&mut h, &["Text: "]);
        // 20 columns, 14 after the prompt.
        h.frame(184., 600.);
        h.keys("C-c p abcdefghijklmnopqrstuvwxyzABCD");
        let mb = h.core.minibuffer().unwrap();
        let text: String = mb.display.lines[0].spans.iter().map(|s| &*s.text).collect();
        assert_eq!(
            (text.as_str(), mb.display.cursor),
            ("rstuvwxyzABCD", Some((0, 13)))
        );
    }

    #[test]
    fn text_keeps_clear_of_a_window_s_sides() {
        let mut h = on(&"x".repeat(30));
        let (lines, _) = shown(&h.frame(100., 600.));
        assert_eq!(lines[0].chars().count(), 8, "9 columns less the wrap's");
    }

    #[test]
    fn the_minibuffer_keeps_clear_of_the_frame_s_sides() {
        let mut h = on("text");
        ask(&mut h, &["Text: "]);
        // 9 columns, 3 after the prompt.
        h.frame(100., 600.);
        h.keys("C-c p abcd");
        let mb = h.core.minibuffer().unwrap();
        let text: String = mb.display.lines[0].spans.iter().map(|s| &*s.text).collect();
        assert_eq!((text.as_str(), mb.display.cursor), ("cd", Some((0, 2))));
    }

    #[test]
    fn the_mode_line_cuts_off_a_last_row_point_never_sits_on() {
        let mut h = on("0\n1\n2\n3\n4");
        let settings = h.core.settings();
        let height = 2.5 * settings.line_height() + settings.mode_line_height();
        assert_eq!(
            shown(&h.frame(800., height)),
            (vec!["0".into(), "1".into(), "2".into()], Some((0, 0)))
        );
        h.keys("C-n C-n");
        let (lines, cursor) = shown(&h.frame(800., height));
        assert_eq!(
            (lines[0].as_str(), cursor),
            ("1", Some((1, 0))),
            "scrolled as for 2 rows"
        );
        let height = 3. * h.core.settings().line_height() + h.core.settings().mode_line_height();
        assert_eq!(
            shown(&h.frame(800., height)).0.len(),
            3,
            "no extra row at whole rows"
        );
    }

    #[test]
    fn resizing_re_lays_out_snapped_to_lines() {
        let mut h = on("one");
        h.keys("C-x 2");
        let line = h.core.settings().line_height();
        let chrome = h.core.settings().mode_line_height();
        // Text heights in lines, to two places.
        let heights = |h: &mut Harness, lines: f32| -> Vec<f32> {
            let frame = h.frame(800., line * lines + 2. * chrome);
            let lines = |w: &FrameWindow| ((w.rect.height - chrome) / line * 100.).round() / 100.;
            frame.windows.iter().map(lines).collect()
        };
        assert_eq!(heights(&mut h, 10.), [5., 5.]);
        assert_eq!(heights(&mut h, 7.2), [4., 3.2]);
    }

    /// `files` with `line_numbers` set to `style`.
    fn numbered(files: &[(&str, &str)], style: &str) -> Harness {
        let mut h = Harness::new(files);
        let settings = format!("line_numbers = \"{style}\"");
        h.core.settings = Settings::parse(Some(&settings)).unwrap();
        h
    }

    /// The first window's gutter labels in a frame `width` px wide, the current line's starred.
    fn labels(h: &mut Harness, width: f32) -> Vec<String> {
        let current = h.core.settings().theme.face(FaceName::LineNumberCurrent);
        let window = h.frame(width, 600.).windows.remove(0);
        (window.gutter.map_or(Vec::new(), |g| g.rows).into_iter())
            .map(|(label, face)| match face == current {
                true => format!("{label}*"),
                false => label,
            })
            .collect()
    }

    #[test]
    fn the_gutter_numbers_a_files_lines_from_one() {
        let mut h = numbered(&[("a.txt", "a\nb\nc")], "absolute");
        h.open("a.txt").keys("C-n");
        assert_eq!(labels(&mut h, 800.), ["1", "2*", "3"]);
    }

    #[test]
    fn relative_numbers_count_from_points_line_which_keeps_its_own() {
        let mut h = numbered(&[("a.txt", "a\nb\nc\nd\ne")], "relative");
        h.open("a.txt").keys("C-n C-n");
        assert_eq!(labels(&mut h, 800.), ["2", "1", "3*", "1", "2"]);
    }

    #[test]
    fn files_scratch_and_messages_have_a_gutter_and_off_turns_it_off() {
        let mut h = numbered(&[("a.txt", "a")], "absolute");
        assert_eq!(labels(&mut h, 800.), ["1*"], "*scratch*");
        h.keys("M-x toggle-truncate-lines RET C-x b *Messages* RET");
        assert!(!labels(&mut h, 800.).is_empty(), "*Messages*");
        h.keys("C-x b other RET");
        assert_eq!(labels(&mut h, 800.), [] as [String; 0], "another buffer");
        let mut h = numbered(&[("a.txt", "a")], "off");
        h.open("a.txt");
        assert_eq!(labels(&mut h, 800.), [] as [String; 0]);
    }

    /// The text of each row of the first window with the `hl-line` background.
    fn highlighted(h: &mut Harness, width: f32) -> Vec<String> {
        let hl = Decoration::Background(FaceName::HlLine);
        let window = h.frame(width, 600.).windows.remove(0);
        (window.display.lines.iter())
            .filter(|l| l.decorations.contains(&hl))
            .map(|l| l.spans.iter().map(|s| s.text.as_str()).collect())
            .collect()
    }

    #[test]
    fn point_s_line_is_highlighted_in_files_only() {
        let mut h = Harness::new(&[("a.txt", "one\ntwo\nthree")]);
        h.open("a.txt").keys("C-n");
        assert_eq!(highlighted(&mut h, 800.), ["two"]);
        h.keys("C-x b other RET");
        assert!(highlighted(&mut h, 800.).is_empty(), "another buffer");
        h.keys("C-x b *scratch* RET");
        assert_eq!(highlighted(&mut h, 800.), [""]);
    }

    #[test]
    fn every_row_of_a_wrapped_line_is_highlighted() {
        let text = format!("{}\nb", "a".repeat(100));
        let mut h = Harness::new(&[("a.rs", &text)]);
        h.open("a.rs");
        let rows = highlighted(&mut h, 400.);
        assert!(rows.len() > 1, "{rows:?}");
        assert!(rows.iter().all(|r| !r.contains('b')));
    }

    #[test]
    fn the_selected_window_alone_is_highlighted() {
        let mut h = Harness::new(&[("a.txt", "one\ntwo")]);
        h.open("a.txt").keys("C-x 2");
        let hl = Decoration::Background(FaceName::HlLine);
        let shown = |h: &mut Harness| {
            (h.frame(800., 600.).windows.iter())
                .map(|w| (w.display.lines[0].decorations.contains(&hl), w.selected))
                .collect::<Vec<_>>()
        };
        assert_eq!(shown(&mut h), [(true, true), (false, false)]);
        h.keys("C-x o");
        assert_eq!(shown(&mut h), [(false, false), (true, true)]);
    }

    #[test]
    fn a_preview_is_highlighted_in_the_window_it_came_from() {
        let mut h = Harness::new(&[("a.txt", "one\ntwo\nthree")]);
        h.open("a.txt").keys("C-s thr");
        assert_eq!(highlighted(&mut h, 800.), ["three"]);
        assert_eq!(
            h.frame(800., 600.).minibuffer.unwrap().display.lines[0].decorations,
            []
        );
    }

    #[test]
    fn the_setting_turns_the_highlight_off() {
        let mut h = Harness::new(&[("a.txt", "one")]);
        h.core.settings = Settings::parse(Some("hl_line = false")).unwrap();
        h.open("a.txt");
        assert!(highlighted(&mut h, 800.).is_empty());
    }

    #[test]
    fn the_command_toggles_one_buffer_over_the_setting() {
        let mut h = numbered(&[("a.txt", "a"), ("b.txt", "b")], "absolute");
        h.open("b.txt").open("a.txt");
        h.keys("M-x display-line-numbers-mode RET");
        let off = "Display-Line-Numbers mode disabled in current buffer";
        assert_eq!(h.core.message(), Some(off));
        assert!(labels(&mut h, 800.).is_empty());
        h.open("b.txt");
        assert_eq!(
            labels(&mut h, 800.),
            ["1*"],
            "the other buffer keeps its own"
        );

        let mut h = numbered(&[("a.txt", "a\nb")], "off");
        h.open("a.txt").keys("M-x display-line-numbers-mode RET");
        let on = "Display-Line-Numbers mode enabled in current buffer";
        assert_eq!(h.core.message(), Some(on));
        assert_eq!(labels(&mut h, 800.), ["1*", "2"], "absolute over off");

        let mut h = numbered(&[("a.txt", "a\nb")], "relative");
        h.open("a.txt");
        h.keys("M-x display-line-numbers-mode RET M-x display-line-numbers-mode RET");
        assert_eq!(
            labels(&mut h, 800.),
            ["1*", "1"],
            "toggling back keeps the style"
        );

        h.keys("C-x b other RET M-x display-line-numbers-mode RET");
        assert_eq!(h.core.message(), Some("No line numbers in this buffer"));

        let mut h = numbered(&[], "absolute");
        h.keys("M-x display-line-numbers-mode RET");
        assert!(labels(&mut h, 800.).is_empty(), "*scratch* toggles too");
    }

    #[test]
    fn the_setting_reaches_buffers_that_were_not_toggled() {
        let mut h = numbered(&[("a.txt", "a\nb")], "absolute");
        h.open("a.txt").keys("C-n");
        let settings = "line_numbers = \"relative\"".to_string();
        h.send(Msg::SettingsLoaded(Ok(Some(settings))));
        assert_eq!(labels(&mut h, 800.), ["1", "2*"]);
    }

    #[test]
    fn wrapped_rows_show_a_marker_and_text_wraps_beside_the_gutter() {
        let mut h = numbered(&[("a.rs", "abcdefghijklmn\nb")], "absolute");
        // 30 px of gutter leaves 10 of 110 px's columns, 9 chars to a row.
        h.open("a.rs");
        assert_eq!(labels(&mut h, 110.), ["1*", "↪", "2"]);
        let frame = h.frame(122., 600.);
        let (window, gutter) = (&frame.windows[0], frame.windows[0].gutter.as_ref());
        assert_eq!(gutter.unwrap().width, 30.);
        assert_eq!(window.display.lines[0].spans[0].text, "abcdefghi");
        assert_eq!(window.display.lines[1].spans[0].text, "jklmn");
        h.keys("C-n");
        assert_eq!(
            labels(&mut h, 110.),
            ["1*", "↪", "2"],
            "point on the second row"
        );
    }

    #[test]
    fn a_window_starting_inside_a_line_starts_with_the_marker() {
        // More rows than the window shows, with point on the last.
        let text = "x".repeat(9 * 45);
        let mut h = numbered(&[("a.rs", &text)], "absolute");
        h.open("a.rs").keys("C-e");
        let rows = labels(&mut h, 110.);
        assert!(rows.len() > 1 && rows.iter().all(|l| l == "↪"), "{rows:?}");
    }

    #[test]
    fn the_gutter_grows_with_the_line_count() {
        let mut h = numbered(&[("a.txt", &"\n".repeat(8))], "absolute");
        h.open("a.txt");
        let width = |h: &mut Harness| {
            h.frame(800., 600.).windows[0]
                .gutter
                .as_ref()
                .unwrap()
                .width
        };
        assert_eq!(width(&mut h), 30.);
        h.keys("RET");
        assert_eq!(width(&mut h), 38.);
    }

    #[test]
    fn a_window_too_narrow_for_it_has_no_gutter() {
        let mut h = numbered(&[("a.txt", "a")], "absolute");
        h.open("a.txt");
        assert!(labels(&mut h, 50.).is_empty(), "one cell left");
        assert_eq!(labels(&mut h, 51.), ["1*"]);
    }
}
