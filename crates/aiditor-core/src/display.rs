//! The display model: what the core shows for a window, for the frontend to draw.

use crate::buffer::{Buffer, Decoration, Face, Prop};
use crate::settings::{FaceName, LineNumbers, Settings, Theme};
use std::ops::Range;

#[derive(Debug, PartialEq)]
pub struct WindowDisplay {
    pub lines: Vec<Line>,
    /// Point as (index into `lines`, char column in that line's text), if it is shown.
    pub cursor: Option<(usize, usize)>,
    pub mode_line: String,
    /// A terminal buffer's screen, drawn instead of `lines` and `cursor`.
    pub terminal: Option<Terminal>,
}

/// A terminal's live screen: its rows' runs and its cursor.
#[derive(Debug, PartialEq)]
pub struct Terminal {
    pub rows: Vec<Vec<Run>>,
    pub cursor: Option<Cursor>,
}

/// Adjacent cells of one style, from grid column `col`; a wide char is a run of its own.
#[derive(Debug, PartialEq)]
pub struct Run {
    pub col: u16,
    pub cells: u16,
    pub text: String,
    /// Colors as 0xRRGGBB; no background is the default one.
    pub fg: u32,
    pub bg: Option<u32>,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
}

#[derive(Debug, PartialEq)]
pub struct Cursor {
    pub row: u16,
    pub col: u16,
    /// Two on a wide char.
    pub cells: u16,
    pub shape: CursorShape,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum CursorShape {
    Block,
    Beam,
    Underline,
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
    /// The which-key hint, once a pending prefix's delay has passed.
    pub hint: Option<Hint>,
    /// The frame's title: the current buffer's name, with ` *` when it is modified, then
    /// ` | <workspace> | <project>` for the current workspace.
    pub title: String,
    /// The workspace modeline's project and workspace names, cut to fit the frame's width.
    pub project: String,
    pub workspace: String,
}

/// The open minibuffer: its prompt and one line, the one holding point.
#[derive(Debug, PartialEq)]
pub struct Minibuffer {
    /// The selected match's number and how many match, as vertico's `1/100 `; empty without completion.
    pub count: String,
    pub prompt: String,
    pub display: WindowDisplay,
    /// The completion list's shown rows, best match first; empty without completion.
    pub candidates: Vec<String>,
    /// Each of `candidates`' annotation, empty for none.
    pub annotations: Vec<String>,
    /// Which of `candidates` is selected.
    pub selected: Option<usize>,
    /// How many rows the completion list takes, the most shown since the minibuffer opened;
    /// rows past `candidates` are drawn empty, so the windows above keep their height.
    pub rows: usize,
}

#[derive(Debug, PartialEq)]
pub struct FrameWindow {
    pub id: crate::window::WindowId,
    pub rect: Rect,
    pub selected: bool,
    pub display: WindowDisplay,
    pub gutter: Option<Gutter>,
}

/// A window's line numbers, drawn in the width its text leaves.
#[derive(Debug, PartialEq)]
pub struct Gutter {
    /// In px; the text starts after it.
    pub width: f32,
    /// A label and its face for each of the window's display lines.
    pub rows: Vec<(String, Face)>,
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

/// How a window wraps long lines: `width` chars to a row, breaking at words or at the width, and
/// shown from char column `from` of its first line, where its first row starts.
#[derive(Clone, Copy, Debug)]
pub struct Wrap {
    pub width: usize,
    pub word: bool,
    pub from: usize,
}

/// The px between a gutter's left edge and its digits, and between them and the text.
const GUTTER_LEFT: f32 = 10.;
pub const GUTTER_RIGHT: f32 = 12.;
/// Between text and a window's side where no gutter stands.
pub const TEXT_INSET: f32 = 12.;

/// The width of a gutter for a buffer of `lines` lines in `cell` px cells.
pub(crate) fn gutter_width(lines: usize, cell: f32) -> f32 {
    GUTTER_LEFT + lines.to_string().len() as f32 * cell + GUTTER_RIGHT
}

/// The gutter of `lines` with point on buffer line `point`. Rows of a wrapped line share its
/// number, and `continued` says the first row is not its line's first.
pub(crate) fn gutter(
    lines: &[Line],
    style: LineNumbers,
    width: f32,
    point: usize,
    continued: bool,
    theme: &Theme,
) -> Gutter {
    let mut previous = lines.first().filter(|_| continued).map(|l| l.number);
    let rows = (lines.iter())
        .map(|line| {
            let n = line.number;
            let (label, face) = if previous.replace(n) == Some(n) {
                ("↪".to_string(), FaceName::LineNumber)
            } else if n == point {
                ((n + 1).to_string(), FaceName::LineNumberCurrent)
            } else if style == LineNumbers::Relative {
                (n.abs_diff(point).to_string(), FaceName::LineNumber)
            } else {
                ((n + 1).to_string(), FaceName::LineNumber)
            };
            (label, theme.face(face))
        })
        .collect();
    Gutter { width, rows }
}

/// Where each visual row of a line of `len` chars starts, `width` chars to a row. At words a row
/// breaks after the last whitespace that fits, one more fitting in the column left for the
/// marker, and at the width when none does.
pub fn row_starts(
    len: usize,
    width: usize,
    word: bool,
    space: impl Fn(usize) -> bool,
) -> Vec<usize> {
    let mut starts = vec![0];
    let mut start = 0;
    loop {
        let rest = len - start;
        if rest <= width || (word && rest == width + 1 && space(len - 1)) {
            return starts;
        }
        let at_word = word.then(|| {
            (start + 1..=start + width + 1)
                .rev()
                .find(|&i| space(i - 1))
        });
        start = at_word.flatten().unwrap_or(start + width);
        starts.push(start);
    }
}

/// Builds `rows` display lines from buffer line `top`, plus the cursor and mode line. Point's line
/// holds its `shown` columns, every other line the first `shown.len()`; with `wrap`, each line is
/// then split into rows.
#[allow(clippy::too_many_arguments)]
pub fn build(
    buffer: &Buffer,
    top: usize,
    rows: usize,
    point: usize,
    mark: Option<usize>,
    settings: &Settings,
    shown: Range<usize>,
    wrap: Option<Wrap>,
) -> WindowDisplay {
    let text = buffer.text();
    let (top_line, end_line) = (
        top.min(text.len_lines() - 1),
        (top + rows + 1).min(text.len_lines()),
    );
    // Rows break greedily, so laying out from the first row's start breaks as the whole line would.
    let from = wrap.map_or(0, |w| w.from);
    let (first, mut last) = (
        text.line_to_char(top_line) + from,
        text.line_to_char(end_line),
    );
    let folds = buffer.has_folds();
    // Only the properties near the window count, which keeps a coloured scrollback in terminal
    // copy mode and a long session transcript cheap to draw. Folds join lines, so the window
    // reaches one more line for each newline they hide in it.
    let mut props = buffer.properties_near(first..last);
    let mut widen = folds;
    while widen {
        let hidden: usize = (props.iter())
            .filter(|(_, p)| matches!(p, Prop::Invisible(_)))
            .map(|(r, _)| {
                text.char_to_line(r.end.min(last)) - text.char_to_line(r.start.max(first))
            })
            .sum();
        let end = text.line_to_char((end_line + hidden).min(text.len_lines()));
        widen = end > last;
        if widen {
            last = end;
            props = buffer.properties_near(first..last);
        }
    }
    // shortcut: point's line is the display line starting on point's buffer line, so one joined by a fold from an earlier line is not scrolled; compare display lines when folds get common.
    let point_line = buffer.line_col(point).ok().map(|(l, _)| l);
    // Without folds a column is a char, so only each line's drawn chars are highlighted.
    // shortcut: only the window's buffer lines are highlighted, so lines a fold joins in are drawn uncoloured; highlight display lines when folds get common.
    let mut drawn: Vec<Range<usize>> = Vec::new();
    if buffer.highlighted() && folds {
        drawn.push(first..last);
    } else if buffer.highlighted() {
        for l in top_line..end_line {
            let start = text.line_to_char(l) + if l == top_line { from } else { 0 };
            let Ok(end) = buffer.char_idx(l, usize::MAX) else {
                continue;
            };
            let cols = match Some(l) == point_line {
                true => shown.clone(),
                false => 0..shown.len(),
            };
            let len = end - start;
            let chars = start + cols.start.min(len)..start + cols.end.min(len);
            // A line drawn to its end and the next drawn from its start are one query, as a
            // query per line costs more than the newline between them.
            match drawn.last_mut() {
                Some(last) if last.end + 1 == start && chars.start == start => last.end = chars.end,
                _ if !chars.is_empty() => drawn.push(chars),
                _ => {}
            }
        }
    }
    let syntax = buffer.highlights(&drawn, &settings.theme.syntax);
    let region = mark.map(|m| m.min(point)..m.max(point));
    let new_line = |number| Line {
        number,
        height: settings.line_height(),
        spans: Vec::new(),
        decorations: Vec::new(),
    };
    let mut lines = Vec::new();
    let mut cursor = None;
    let mut pos = first;
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
    // Where a run of plain chars from `pos` ends: at its line's newline, or a fold that may join
    // the next line.
    let stop = |pos: usize| {
        let l = text.char_to_line(pos);
        let newline = match l + 1 < text.len_lines() {
            true => text.line_to_char(l + 1) - 1,
            false => text.len_chars(),
        };
        (props.iter())
            .filter(|(r, p)| matches!(p, Prop::Invisible(_)) && (pos..newline).contains(&r.start))
            .map(|(r, _)| r.start)
            .fold(newline, usize::min)
    };
    // Display columns of `line` consumed so far, the skipped ones included.
    let mut col: usize = 0;
    while lines.len() < rows {
        let range = match Some(line.number) == point_line {
            true => shown.clone(),
            false => 0..shown.len(),
        };
        let fold = props.iter().find_map(|(range, prop)| match prop {
            Prop::Invisible(placeholder) if range.contains(&pos) => Some((range.end, placeholder)),
            _ => None,
        });
        if point == pos || fold.is_some_and(|(end, _)| point > pos && point < end) {
            cursor = Some((lines.len(), col.saturating_sub(range.start)));
        }
        if let Some((end, placeholder)) = fold {
            let visible: String = (placeholder.chars().enumerate())
                .filter(|(i, _)| range.contains(&(col + i)))
                .map(|(_, c)| c)
                .collect();
            col += placeholder.chars().count();
            if !visible.is_empty() {
                line.spans.push(Span {
                    text: visible,
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
        if col >= range.end {
            // The line is full: skip to its newline, or to a fold that may join the next line.
            let target = stop(pos);
            if target > pos {
                pos = target;
                continue;
            }
        }
        if col < range.start {
            // Columns scrolled off to the left: skip them too, stopping at point to place the cursor.
            let mut target = stop(pos).min(pos + range.start - col);
            if point > pos {
                target = target.min(point);
            }
            if target > pos {
                col += target - pos;
                pos = target;
                continue;
            }
        }
        let c = text.char(pos);
        pos += 1;
        if c == '\n' {
            finish(&mut lines, &mut line, start, pos, text.char_to_line(pos));
            start = pos;
            col = 0;
            continue;
        }
        col += 1;
        if !range.contains(&(col - 1)) {
            continue;
        }
        let i = syntax.partition_point(|(range, _)| range.end < pos);
        let mut face = (syntax.get(i))
            .filter(|(range, _)| range.start < pos)
            .map_or(Face::default(), |(_, face)| *face);
        for (range, prop) in &props {
            if !range.contains(&(pos - 1)) {
                continue;
            }
            match prop {
                Prop::Face(f) => face = f.over(face),
                Prop::Named(refs) => face = settings.theme.resolve(refs).over(face),
                _ => {}
            }
        }
        if region.as_ref().is_some_and(|r| r.contains(&(pos - 1))) {
            face = settings.theme.face(FaceName::Region).over(face);
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
    if let Some(wrap) = wrap {
        let marker = crate::syntax::face(&settings.theme.syntax, "comment").unwrap_or_default();
        lines = wrap_lines(lines, &mut cursor, rows, wrap, marker);
    }
    let (l, c) = buffer.line_col(point).unwrap_or_default();
    let state = match (buffer.read_only, buffer.modified()) {
        (true, _) => "%%",
        (_, true) => "**",
        _ => "--",
    };
    let dos = if buffer.crlf { " (DOS)" } else { "" };
    let disk = match (buffer.deleted, buffer.conflict) {
        (true, _) => " [deleted]",
        (_, Some(_)) => " [conflict]",
        _ => "",
    };
    let position = format!("  {}:{c}", l + 1);
    // A terminal in char mode has no point to show.
    let (mode, position) = match buffer.terminal() {
        Some(_) if buffer.copy_mode() => (" [copy]", position),
        Some(_) => (" [char]", String::new()),
        None => ("", position),
    };
    let agent = buffer.agent_mode.as_ref().map(|m| format!(" [{m}]"));
    WindowDisplay {
        lines,
        cursor,
        // A buffer name may hold newlines; the mode line is one line, so show them as Emacs does.
        mode_line: format!(
            "{state} {}{dos}{disk}{mode}{}{position}",
            buffer.name.replace('\n', "^J"),
            agent.unwrap_or_default(),
        ),
        terminal: None,
    }
}

/// Splits each of `lines` into its rows, ending each row but the last with a `\` in `marker`
/// when not breaking at words, and keeps the first `rows` of them, the cursor moving along.
fn wrap_lines(
    lines: Vec<Line>,
    cursor: &mut Option<(usize, usize)>,
    rows: usize,
    wrap: Wrap,
    marker: Face,
) -> Vec<Line> {
    let mut out = Vec::new();
    let mut moved = None;
    for (i, line) in lines.into_iter().enumerate() {
        let chars: Vec<char> = line.spans.iter().flat_map(|s| s.text.chars()).collect();
        let starts = row_starts(chars.len(), wrap.width, wrap.word, |i| {
            chars[i].is_whitespace()
        });
        let row_of = |col: usize| starts.partition_point(|&s| s <= col) - 1;
        if let Some((_, col)) = cursor.filter(|&(l, _)| l == i) {
            moved = Some((out.len() + row_of(col), col - starts[row_of(col)]));
        }
        let mut split: Vec<Vec<Span>> = starts.iter().map(|_| Vec::new()).collect();
        let mut col = 0;
        for span in line.spans {
            let mut row = row_of(col);
            let mut text = String::new();
            for c in span.text.chars() {
                if starts.get(row + 1) == Some(&col) {
                    split[row].push(Span {
                        text: std::mem::take(&mut text),
                        ..span
                    });
                    row += 1;
                }
                text.push(c);
                col += 1;
            }
            if !text.is_empty() {
                split[row].push(Span { text, ..span });
            }
        }
        let last = split.len() - 1;
        for (row, mut spans) in split.into_iter().enumerate() {
            if !wrap.word && row < last {
                spans.push(Span {
                    text: "\\".into(),
                    face: marker,
                    fold: false,
                });
            }
            out.push(Line {
                number: line.number,
                height: line.height,
                spans,
                decorations: line.decorations.clone(),
            });
        }
    }
    *cursor = moved.filter(|&(r, _)| r < rows);
    out.truncate(rows);
    out
}

#[cfg(test)]
mod tests {
    use crate::buffer::{Decoration, Face, Prop};
    use crate::harness::Harness;
    use crate::settings::{FaceName, Settings};

    const RED: Face = Face {
        fg: Some(0xff0000),
        bg: None,
        bold: false,
        italic: false,
        underline: false,
    };

    fn prop(h: &mut Harness, range: std::ops::Range<usize>, prop: Prop) {
        let id = h.core.windows.window.buffer();
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
    fn rows_break_after_whitespace_and_one_may_take_the_last_column() {
        let starts = |text: &str, word| {
            let chars: Vec<char> = text.chars().collect();
            super::row_starts(chars.len(), 8, word, |i| chars[i].is_whitespace())
        };
        assert_eq!(
            starts("abcdefgh ij", true),
            [0, 9],
            "a space in the last column"
        );
        assert_eq!(
            starts("abcdefgh ", true),
            [0],
            "a line ending in it is one row"
        );
        assert_eq!(starts("ab cdefghij", true), [0, 3]);
        assert_eq!(starts("abcdefghij", true), [0, 8], "no whitespace");
        assert_eq!(starts("abcdefgh ij", false), [0, 8], "at the edge");
        assert_eq!(starts("", false), [0]);
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
            ..Face::default()
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
        prop(
            &mut h,
            6..7,
            Prop::Line(Decoration::Rule(FaceName::SessionThought)),
        );
        insta::assert_debug_snapshot!(h.display(5));
    }

    #[test]
    fn folds_reach_lines_past_the_window_and_their_properties() {
        let mut h = Harness::new(&[]);
        h.keys("a RET b RET c RET d RET e RET f M-<");
        // `a…` hides `b` to `d`; the window's 2 rows then reach `e` and its face, then `f`.
        prop(&mut h, 1..7, Prop::Invisible("…".into()));
        prop(&mut h, 8..9, Prop::Face(RED));
        prop(&mut h, 9..10, Prop::Invisible(String::new()));
        let display = h.display(2);
        assert_eq!(texts(&mut h, 2), ["a…", "ef"]);
        assert_eq!(display.lines[1].spans[0].face, RED);
    }

    #[test]
    fn hidden_markup_and_the_cursor_after_it() {
        let mut h = Harness::new(&[]);
        h.keys("**bold**");
        prop(&mut h, 0..2, Prop::Invisible(String::new()));
        prop(&mut h, 6..8, Prop::Invisible(String::new()));
        assert_eq!(texts(&mut h, 1), ["bold"]);
        assert_eq!(h.display(1).cursor, Some((0, 4)), "point at the end");
        h.core
            .windows
            .window
            .set_point(&mut h.core.buffers, 1)
            .unwrap();
        assert_eq!(h.display(1).cursor, Some((0, 0)), "point inside markup");
    }

    #[test]
    fn cursor_after_a_placeholder() {
        let mut h = Harness::new(&[]);
        h.keys("abcdef");
        prop(&mut h, 0..3, Prop::Invisible("…".into()));
        h.core
            .windows
            .window
            .set_point(&mut h.core.buffers, 3)
            .unwrap();
        assert_eq!(h.display(1).cursor, Some((0, 1)));
        h.core
            .windows
            .window
            .set_point(&mut h.core.buffers, 2)
            .unwrap();
        assert_eq!(h.display(1).cursor, Some((0, 0)), "inside the fold");
    }

    #[test]
    fn decorations_cover_the_lines_they_touch() {
        let mut h = Harness::new(&[]);
        h.keys("a RET b RET c RET d");
        prop(
            &mut h,
            2..5,
            Prop::Line(Decoration::Background(FaceName::SessionBlock)),
        );
        prop(
            &mut h,
            6..7,
            Prop::Line(Decoration::Rule(FaceName::SessionThought)),
        );
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
                vec![Decoration::Background(FaceName::SessionBlock)],
                vec![Decoration::Background(FaceName::SessionBlock)],
                vec![Decoration::Rule(FaceName::SessionThought)],
            ]
        );
    }

    #[test]
    fn region_between_point_and_mark() {
        let mut h = Harness::new(&[]);
        h.keys("hello <left> <left>");
        let region = h.core.settings().theme.face(FaceName::Region).bg;
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
            .windows
            .window
            .push_mark(&mut h.core.buffers, 0, true)
            .unwrap();
        assert_eq!(spans(&mut h), [("hel".into(), region), ("lo".into(), None)]);
        h.core.windows.window.set_active(false);
        assert_eq!(
            spans(&mut h),
            [("hello".into(), None)],
            "inactive mark, no region"
        );
        h.core.windows.window.set_active(true);
        prop(&mut h, 1..5, Prop::Face(RED));
        insta::assert_debug_snapshot!(h.display(1).lines);
    }

    #[test]
    fn syntax_entries_are_whole_faces() {
        let mut h = Harness::new(&[("a.js", "// x")]);
        let settings = "[themes.default.syntax]\ncomment = { fg = 7, italic = true }";
        h.core.settings = Settings::parse(Some(settings)).unwrap();
        h.open("a.js");
        let face = h.display(1).lines.remove(0).spans[0].face;
        assert_eq!((face.fg, face.italic), (Some(7), true));
    }

    /// Each span of the first `rows` lines, as (text, foreground).
    fn colors(h: &mut Harness, rows: usize) -> Vec<(String, Option<u32>)> {
        (h.display(rows).lines.into_iter())
            .flat_map(|l| l.spans)
            .map(|s| (s.text, s.face.fg))
            .collect()
    }

    #[test]
    fn code_is_coloured_and_follows_edits_undo_and_reverts() {
        let mut h = Harness::new(&[("a.js", "if (x) f(1);")]);
        h.open("a.js");
        let syntax = h.core.settings().theme.syntax.clone();
        let c = |name: &str| syntax[name].fg;
        let code = vec![
            ("if".into(), c("keyword")),
            (" (".into(), None),
            ("x".into(), c("variable")),
            (") ".into(), None),
            ("f".into(), c("function")),
            ("(".into(), None),
            ("1".into(), c("number")),
            (");".into(), None),
        ];
        assert_eq!(colors(&mut h, 1), code);
        h.keys("/ / SPC");
        assert_eq!(
            colors(&mut h, 1),
            [("// if (x) f(1);".into(), c("comment"))]
        );
        h.keys("C-/");
        assert_eq!(colors(&mut h, 1), code);
        h.prepare_revert("a.js", Some(b"'s'"));
        h.keys("M-x revert-buffer RET y RET");
        assert_eq!(colors(&mut h, 1), [("'s'".into(), c("string"))]);
    }

    #[test]
    fn other_files_and_the_region_over_code() {
        let mut h = Harness::new(&[("a.txt", "if (x) f(1);"), ("b.ts", "let x")]);
        h.open("a.txt");
        assert_eq!(colors(&mut h, 1), [("if (x) f(1);".into(), None)]);
        h.open("b.ts").keys("C-SPC C-f C-f");
        let theme = h.core.settings().theme.clone();
        let spans = h.display(1).lines.remove(0).spans;
        let faces: Vec<_> = spans.iter().map(|s| (s.face.fg, s.face.bg)).collect();
        let keyword = theme.syntax["keyword"].fg;
        assert_eq!(faces[0], (keyword, theme.face(FaceName::Region).bg));
    }

    #[test]
    fn a_scrolled_window_is_highlighted_from_its_top_line() {
        let text = "let a;\n".repeat(1000);
        let mut h = Harness::new(&[("a.js", &text)]);
        h.open("a.js").keys("M->");
        assert_eq!(h.display(3).lines[0].number, 998);
        let syntax = &h.core.settings().theme.syntax;
        let (keyword, variable) = (syntax["keyword"].fg, syntax["variable"].fg);
        let line = [
            ("let".into(), keyword),
            (" ".into(), None),
            ("a".into(), variable),
            (";".into(), None),
        ];
        assert_eq!(colors(&mut h, 3), [line.clone(), line].concat());
    }

    /// Each line's text and the cursor, built with point's line showing columns `shown`.
    fn scrolled(
        h: &Harness,
        point: usize,
        shown: std::ops::Range<usize>,
    ) -> (Vec<String>, Option<(usize, usize)>) {
        let d = super::build(
            h.core.current(),
            0,
            2,
            point,
            None,
            h.core.settings(),
            shown,
            None,
        );
        let texts = (d.lines.iter())
            .map(|l| l.spans.iter().map(|s| s.text.as_str()).collect())
            .collect();
        (texts, d.cursor)
    }

    #[test]
    fn a_scrolled_line_skips_its_hidden_columns_and_folds() {
        let mut h = Harness::new(&[]);
        h.keys("abcdefghij RET xyz");
        prop(&mut h, 2..4, Prop::Invisible("…".into()));
        // Columns: a b … e f g h i j, the fold one wide.
        let lines = vec!["ghi".to_string(), "xyz".into()];
        assert_eq!(scrolled(&h, 8, 5..8), (lines.clone(), Some((0, 2))));
        assert_eq!(
            scrolled(&h, 0, 5..8),
            (lines, Some((0, 0))),
            "point scrolled off"
        );
        let (texts, cursor) = scrolled(&h, 3, 1..4);
        assert_eq!(
            (texts[0].as_str(), cursor),
            ("b…e", Some((0, 1))),
            "point in the fold"
        );
    }

    #[test]
    fn a_scrolled_long_line_keeps_its_strings_colour() {
        let text = format!("let s = \"{}\"; let t;", "x".repeat(1000));
        let mut h = Harness::new(&[("a.js", &text)]);
        h.open("a.js");
        let string = h.core.settings().theme.syntax["string"].fg;
        let d = super::build(
            h.core.current(),
            0,
            1,
            500,
            None,
            h.core.settings(),
            495..505,
            None,
        );
        let spans: Vec<_> = (d.lines[0].spans.iter())
            .map(|s| (s.text.as_str(), s.face.fg))
            .collect();
        assert_eq!(spans, [("xxxxxxxxxx", string)]);
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
