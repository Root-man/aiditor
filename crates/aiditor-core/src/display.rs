//! The display model: what the core shows for a window, for the frontend to draw.

use crate::buffer::{Buffer, Decoration, Face, Prop};
use crate::settings::Settings;

#[derive(Debug, PartialEq)]
pub struct WindowDisplay {
    pub lines: Vec<Line>,
    /// Point as (index into `lines`, char column in that line's text), if it is shown.
    pub cursor: Option<(usize, usize)>,
    pub mode_line: String,
}

/// A rectangle of the frame in pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

/// Every window of the frame, laid out by the split tree.
#[derive(Debug, PartialEq)]
pub struct FrameDisplay {
    pub windows: Vec<FrameWindow>,
    /// The echo area's text, if a message is shown.
    pub message: Option<String>,
    /// The open minibuffer, if any.
    pub minibuffer: Option<Minibuffer>,
}

/// The open minibuffer: its prompt and one line, the one holding point.
#[derive(Debug, PartialEq)]
pub struct Minibuffer {
    pub prompt: String,
    pub display: WindowDisplay,
    /// The completion list's shown rows, best match first; empty without completion.
    pub candidates: Vec<String>,
    /// Each of `candidates`' annotation, empty for none.
    pub annotations: Vec<String>,
    /// Which of `candidates` is selected.
    pub selected: Option<usize>,
}

#[derive(Debug, PartialEq)]
pub struct FrameWindow {
    pub id: crate::window::WindowId,
    pub rect: Rect,
    pub selected: bool,
    pub display: WindowDisplay,
}

/// The which-key hint shown while a prefix waits: the pending keys and what can follow them.
#[derive(Debug, PartialEq)]
pub struct Hint {
    /// The pending keys in `kbd` form.
    pub keys: String,
    /// Each next key and its command name, or `+prefix`, sorted by key.
    pub entries: Vec<(String, String)>,
}

/// One display line: a buffer line, or several joined by a fold.
#[derive(Debug, PartialEq)]
pub struct Line {
    /// The buffer line it starts on.
    pub number: usize,
    pub height: f32,
    pub spans: Vec<Span>,
    pub decorations: Vec<Decoration>,
}

#[derive(Debug, PartialEq)]
pub struct Span {
    pub text: String,
    pub face: Face,
    /// The placeholder standing in for invisible text.
    pub fold: bool,
}

/// Builds `rows` display lines from buffer line `top`, plus the cursor and mode line.
// shortcut: lines are emitted whole and the frontend clips them; soft wrap (#112) cuts them here.
pub fn build(
    buffer: &Buffer,
    top: usize,
    rows: usize,
    point: usize,
    mark: Option<usize>,
    settings: &Settings,
) -> WindowDisplay {
    let text = buffer.text();
    // shortcut: every char scans every property, fine for a screenful; index them when buffers get many.
    let props: Vec<_> = buffer.properties().collect();
    let region = mark.map(|m| m.min(point)..m.max(point));
    let new_line = |number| Line {
        number,
        height: settings.line_height(),
        spans: Vec::new(),
        decorations: Vec::new(),
    };
    let mut lines = Vec::new();
    let mut cursor = None;
    let mut pos = text.line_to_char(top.min(text.len_lines() - 1));
    let mut line = new_line(text.char_to_line(pos));
    let mut start = pos;
    // Ends `line` at `end`, giving it every decoration whose range touches start..end.
    let finish = |lines: &mut Vec<Line>, line: &mut Line, start: usize, end: usize, next| {
        let end = end.max(start + 1);
        for (range, prop) in &props {
            if let Prop::Line(d) = prop
                && range.start < end
                && range.end > start
            {
                line.decorations.push(*d);
            }
        }
        lines.push(std::mem::replace(line, new_line(next)));
    };
    let col = |line: &Line| line.spans.iter().map(|s| s.text.chars().count()).sum();
    while lines.len() < rows {
        let fold = props.iter().find_map(|(range, prop)| match prop {
            Prop::Invisible(placeholder) if range.contains(&pos) => Some((range.end, placeholder)),
            _ => None,
        });
        if point == pos || fold.is_some_and(|(end, _)| point > pos && point < end) {
            cursor = Some((lines.len(), col(&line)));
        }
        if let Some((end, placeholder)) = fold {
            if !placeholder.is_empty() {
                line.spans.push(Span {
                    text: placeholder.clone(),
                    face: Face::default(),
                    fold: true,
                });
            }
            pos = end;
            continue;
        }
        if pos == text.len_chars() {
            finish(&mut lines, &mut line, start, pos, 0);
            break;
        }
        let c = text.char(pos);
        pos += 1;
        if c == '\n' {
            finish(&mut lines, &mut line, start, pos, text.char_to_line(pos));
            start = pos;
            continue;
        }
        let mut face = Face::default();
        for (range, prop) in &props {
            if let Prop::Face(f) = prop
                && range.contains(&(pos - 1))
            {
                face.fg = f.fg.or(face.fg);
                face.bg = f.bg.or(face.bg);
            }
        }
        if region.as_ref().is_some_and(|r| r.contains(&(pos - 1))) {
            face.bg = Some(settings.theme.region);
        }
        match line.spans.last_mut() {
            Some(span) if span.face == face && !span.fold => span.text.push(c),
            _ => line.spans.push(Span {
                text: c.into(),
                face,
                fold: false,
            }),
        }
    }
    let (l, c) = buffer.line_col(point).unwrap_or_default();
    let state = match (buffer.read_only, buffer.modified()) {
        (true, _) => "%%",
        (_, true) => "**",
        _ => "--",
    };
    let dos = if buffer.crlf { " (DOS)" } else { "" };
    WindowDisplay {
        lines,
        cursor,
        // A buffer name may hold newlines; the mode line is one line, so show them as Emacs does.
        mode_line: format!(
            "{state} {}{dos}  {}:{c}",
            buffer.name.replace('\n', "^J"),
            l + 1
        ),
    }
}

#[cfg(test)]
mod tests {
    use crate::buffer::{Decoration, Face, Prop};
    use crate::harness::Harness;

    const RED: Face = Face {
        fg: Some(0xff0000),
        bg: None,
    };

    fn prop(h: &mut Harness, range: std::ops::Range<usize>, prop: Prop) {
        let id = h.core.window.buffer();
        let buffer = h.core.buffers.get_mut(id).unwrap();
        buffer.add_property(range, prop).unwrap();
    }

    /// Each display line's text, folds included.
    fn texts(h: &mut Harness, rows: usize) -> Vec<String> {
        h.display(rows)
            .lines
            .iter()
            .map(|l| l.spans.iter().map(|s| s.text.as_str()).collect())
            .collect()
    }

    #[test]
    fn a_short_buffer() {
        let mut h = Harness::new(&[("a.txt", "one\ntwo\nthree")]);
        h.open("a.txt").keys("<down> <right> <right>");
        insta::assert_debug_snapshot!(h.display(10));
    }

    #[test]
    fn a_long_buffer_scrolls_to_point() {
        let mut h = Harness::new(&[("a.txt", "0\n1\n2\n3\n4\n5\n6")]);
        h.open("a.txt").keys("<down> <down> <down> <down> <down>");
        let d = h.display(3);
        let numbers: Vec<_> = d.lines.iter().map(|l| l.number).collect();
        assert_eq!((numbers, d.cursor), (vec![3, 4, 5], Some((2, 0))));
    }

    #[test]
    fn an_empty_buffer_has_one_line() {
        let mut h = Harness::new(&[]);
        insta::assert_debug_snapshot!(h.display(3));
    }

    #[test]
    fn faces_overlap_and_the_later_wins() {
        let mut h = Harness::new(&[]);
        h.keys("hello SPC world");
        let blue_bg = Face {
            fg: Some(0x0000ff),
            bg: Some(0x111111),
        };
        prop(&mut h, 6..11, Prop::Face(RED));
        prop(&mut h, 8..9, Prop::Face(blue_bg));
        insta::assert_debug_snapshot!(h.display(1).lines);
    }

    #[test]
    fn a_fold_joins_lines() {
        let mut h = Harness::new(&[]);
        h.keys("a RET b RET c RET d RET e");
        prop(&mut h, 3..7, Prop::Invisible("…".into()));
        prop(&mut h, 6..7, Prop::Line(Decoration::Rule(0x888888)));
        insta::assert_debug_snapshot!(h.display(5));
    }

    #[test]
    fn hidden_markup_and_the_cursor_after_it() {
        let mut h = Harness::new(&[]);
        h.keys("**bold**");
        prop(&mut h, 0..2, Prop::Invisible(String::new()));
        prop(&mut h, 6..8, Prop::Invisible(String::new()));
        assert_eq!(texts(&mut h, 1), ["bold"]);
        assert_eq!(h.display(1).cursor, Some((0, 4)), "point at the end");
        h.core.window.set_point(&mut h.core.buffers, 1).unwrap();
        assert_eq!(h.display(1).cursor, Some((0, 0)), "point inside markup");
    }

    #[test]
    fn cursor_after_a_placeholder() {
        let mut h = Harness::new(&[]);
        h.keys("abcdef");
        prop(&mut h, 0..3, Prop::Invisible("…".into()));
        h.core.window.set_point(&mut h.core.buffers, 3).unwrap();
        assert_eq!(h.display(1).cursor, Some((0, 1)));
        h.core.window.set_point(&mut h.core.buffers, 2).unwrap();
        assert_eq!(h.display(1).cursor, Some((0, 0)), "inside the fold");
    }

    #[test]
    fn decorations_cover_the_lines_they_touch() {
        let mut h = Harness::new(&[]);
        h.keys("a RET b RET c RET d");
        prop(&mut h, 2..5, Prop::Line(Decoration::Background(0x222222)));
        prop(&mut h, 6..7, Prop::Line(Decoration::Rule(0x888888)));
        let decorations: Vec<_> = h
            .display(4)
            .lines
            .into_iter()
            .map(|l| l.decorations)
            .collect();
        assert_eq!(
            decorations,
            [
                vec![],
                vec![Decoration::Background(0x222222)],
                vec![Decoration::Background(0x222222)],
                vec![Decoration::Rule(0x888888)],
            ]
        );
    }

    #[test]
    fn region_between_point_and_mark() {
        let mut h = Harness::new(&[]);
        h.keys("hello <left> <left>");
        let region = Some(h.core.settings().theme.region);
        let spans = |h: &mut Harness| {
            let line = h.display(1).lines.remove(0);
            line.spans
                .into_iter()
                .map(|s| (s.text, s.face.bg))
                .collect::<Vec<_>>()
        };
        assert_eq!(
            spans(&mut h),
            [("hello".into(), None)],
            "no mark, no region"
        );
        h.core
            .window
            .push_mark(&mut h.core.buffers, 0, true)
            .unwrap();
        assert_eq!(spans(&mut h), [("hel".into(), region), ("lo".into(), None)]);
        h.core.window.set_active(false);
        assert_eq!(
            spans(&mut h),
            [("hello".into(), None)],
            "inactive mark, no region"
        );
        h.core.window.set_active(true);
        prop(&mut h, 1..5, Prop::Face(RED));
        insta::assert_debug_snapshot!(h.display(1).lines);
    }

    #[test]
    fn mode_line_shows_state_name_and_position() {
        let mut h = Harness::new(&[("a.txt", "one\ntwo\nthree")]);
        h.open("a.txt");
        assert_eq!(h.display(5).mode_line, "-- a.txt  1:0");
        h.keys("<down> <down> <right> x");
        assert_eq!(h.display(5).mode_line, "** a.txt  3:2");
    }

    #[test]
    fn mode_line_shows_newlines_in_a_buffer_name_as_caret_j() {
        let mut h = Harness::new(&[]);
        h.keys("C-x b a C-o RET");
        assert_eq!(h.display(5).mode_line, "-- a^J  1:0");
    }
}
