use std::time::Instant;

use aiditor_core::Msg;
use aiditor_core::buffer::{Decoration, Face};
use aiditor_core::display::{
    CursorShape, FrameWindow, GUTTER_RIGHT, Line, Rect, Span, TEXT_INSET, Terminal,
};
use aiditor_core::settings::{FaceName, Theme};
use gpui::{
    App, BorderStyle, Bounds, ContentMask, Element, ElementId, ElementInputHandler, Entity, Font,
    FontStyle, FontWeight, GlobalElementId, InspectorElementId, LayoutId, Pixels, Point,
    ShapedLine, Style, TextRun, UnderlineStyle, Window, fill, outline, point, prelude::*, px, rgb,
    size,
};

use crate::editor::Editor;

/// A window as painted: where, its rows and mode line laid out, and the cursor.
pub(crate) struct Painted {
    pub(crate) bounds: Bounds<Pixels>,
    pub(crate) selected: bool,
    /// Where its text starts, past the gutter, from the window's left.
    pub(crate) text_x: Pixels,
    pub(crate) rows: Vec<Row>,
    /// A terminal's runs, each placed on its grid column.
    pub(crate) cells: Vec<Cells>,
    /// Its mode line's background and an inactive one's rule, each with its colour.
    pub(crate) mode_line_fill: Vec<(Bounds<Pixels>, u32)>,
    pub(crate) mode_line: ShapedLine,
    pub(crate) mode_line_right: ShapedLine,
    pub(crate) cursor: Option<Bounds<Pixels>>,
    /// The char under the cursor, shaped in the background color to show through the block.
    cursor_char: Option<ShapedLine>,
    /// A terminal cursor in a window that isn't selected, drawn as an outline.
    pub(crate) hollow: bool,
}

impl Painted {
    /// Where the mode line's right group starts, so that it ends at the window's right inset.
    pub(crate) fn mode_line_right_x(&self) -> Pixels {
        self.bounds.right() - px(TEXT_INSET) - self.mode_line_right.width
    }
}

/// A terminal run as painted: where, its cells' background and its text.
pub(crate) struct Cells {
    pub(crate) origin: Point<Pixels>,
    background: Option<(Bounds<Pixels>, u32)>,
    pub(crate) text: ShapedLine,
}

pub(crate) struct Row {
    /// Its top, from the window's.
    pub(crate) top: Pixels,
    pub(crate) height: Pixels,
    decorations: Vec<Decoration>,
    /// Its label in the gutter, if the window has one, and the label's x from the window's left.
    pub(crate) number: Option<(Pixels, ShapedLine)>,
    pub(crate) text: ShapedLine,
}

/// Draws the core's frame display: every window's lines, cursor and mode line.
pub(crate) struct FrameElement {
    pub(crate) editor: Entity<Editor>,
    /// The windows from the frame display `render` built, until prepaint shapes them.
    pub(crate) windows: Vec<FrameWindow>,
}

impl IntoElement for FrameElement {
    type Element = Self;

    fn into_element(self) -> Self {
        self
    }
}

impl Element for FrameElement {
    type RequestLayoutState = ();
    type PrepaintState = Vec<Painted>;

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, ()) {
        let style = Style {
            flex_grow: 1.,
            ..Default::default()
        };
        (window.request_layout(style, [], cx), ())
    }

    /// Tells the core the size, rebuilding the windows if it changed, then lays out their lines.
    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) -> Vec<Painted> {
        self.editor.update(cx, |editor, cx| {
            let (font, cell) = editor.font(window);
            let size = (bounds.size.width, bounds.size.height, cell);
            if editor.reported != size {
                editor.reported = size;
                let (width, height, cell_width) = (size.0.into(), size.1.into(), cell.into());
                let msg = Msg::Resize {
                    width,
                    height,
                    cell_width,
                };
                editor.handle(msg, cx);
                self.windows = editor.core.frame().windows;
            }
            let start = Instant::now();
            let settings = editor.core.settings();
            let font_size = px(settings.font_size);
            let shape = |text: String, runs: &[TextRun]| {
                (window.text_system()).shape_line(text.into(), font_size, runs, None)
            };
            let line_height = px(settings.line_height());
            let painted = std::mem::take(&mut self.windows).into_iter().map(|w| {
                let bounds = to_bounds(w.rect, bounds.origin);
                let text_x = px(w.gutter.as_ref().map_or(TEXT_INSET, |g| g.width));
                let mut top = px(0.);
                let mut cursor = None;
                let mut cursor_char = None;
                let rows = (w.display.lines.iter().enumerate()).map(|(i, line)| {
                    let height = px(line.height);
                    let at = w.display.cursor.filter(|&(l, _)| l == i && w.selected);
                    let marked = at.and(editor.marked.as_deref()).unwrap_or_default();
                    let (text, runs, byte) =
                        line_runs(line, at.map(|(_, col)| col), marked, &font, &settings.theme);
                    let text = shape(text, &runs);
                    if at.is_some() {
                        let (x, width, under) = block(&text, byte, cell);
                        let origin = bounds.origin + point(text_x + x, top);
                        cursor = Some(Bounds::new(origin, gpui::size(width, height)));
                        let face = Face {
                            fg: settings.theme.face(FaceName::Cursor).fg,
                            ..Face::default()
                        };
                        let run = text_run(under.len(), face, false, &font, &settings.theme);
                        cursor_char = Some(shape(under.to_string(), &[run]));
                    }
                    let number = w.gutter.as_ref().map(|g| {
                        let (label, face) = &g.rows[i];
                        let run = text_run(label.len(), *face, false, &font, &settings.theme);
                        let label = shape(label.clone(), &[run]);
                        (text_x - px(GUTTER_RIGHT) - label.width, label)
                    });
                    let row = Row {
                        top,
                        height,
                        decorations: line.decorations.clone(),
                        number,
                        text,
                    };
                    top += height;
                    row
                });
                let rows = rows.collect();
                let mut cells = Vec::new();
                let mut hollow = false;
                if let Some(terminal) = &w.display.terminal {
                    let grid = |row: u16, col: u16| {
                        let x = text_x + cell * col as f32;
                        bounds.origin + point(x, line_height * row as f32)
                    };
                    for (row, runs) in terminal.rows.iter().enumerate() {
                        for r in runs {
                            let origin = grid(row as u16, r.col);
                            let run = TextRun {
                                len: r.text.len(),
                                font: styled(&font, r.bold, r.italic),
                                color: rgb(r.fg).into(),
                                ..text_run(0, Face::default(), r.underline, &font, &settings.theme)
                            };
                            let width = cell * r.cells as f32;
                            cells.push(Cells {
                                origin,
                                background: r.bg.map(|bg| {
                                    (Bounds::new(origin, gpui::size(width, line_height)), bg)
                                }),
                                // Each glyph advances one cell, so fallback fonts stay on the grid.
                                text: window.text_system().shape_line(
                                    r.text.clone().into(),
                                    font_size,
                                    &[run],
                                    Some(cell),
                                ),
                            });
                        }
                    }
                    if let Some(c) = &terminal.cursor {
                        let block = Bounds::new(
                            grid(c.row, c.col),
                            gpui::size(cell * c.cells as f32, line_height),
                        );
                        hollow = !w.selected;
                        cursor = Some(match (hollow, c.shape) {
                            (false, CursorShape::Beam) => {
                                Bounds::new(block.origin, gpui::size(px(2.), line_height))
                            }
                            (false, CursorShape::Underline) => Bounds::new(
                                block.bottom_left() - point(px(0.), px(2.)),
                                gpui::size(block.size.width, px(2.)),
                            ),
                            _ => block,
                        });
                        if !hollow && c.shape == CursorShape::Block {
                            cursor_char = under_cursor(terminal, c.row, c.col).map(|text| {
                                let face = Face {
                                    fg: settings.theme.face(FaceName::Cursor).fg,
                                    ..Face::default()
                                };
                                let run = text_run(text.len(), face, false, &font, &settings.theme);
                                (window.text_system()).shape_line(
                                    text.into(),
                                    font_size,
                                    &[run],
                                    Some(cell),
                                )
                            });
                        }
                    }
                }
                let line_face = match w.selected {
                    true => FaceName::ModeLine,
                    false => FaceName::ModeLineInactive,
                };
                let group = |spans: &[Span]| {
                    let text: String = spans.iter().map(|s| s.text.as_str()).collect();
                    let run =
                        |s: &Span| text_run(s.text.len(), s.face, false, &font, &settings.theme);
                    shape(text, &spans.iter().map(run).collect::<Vec<_>>())
                };
                let height = px(settings.mode_line_height());
                let bar = Bounds::new(
                    point(bounds.left(), bounds.bottom() - height),
                    gpui::size(bounds.size.width, height),
                );
                let bar_fill = settings.theme.face(line_face).bg.map(|bg| (bar, bg));
                let rule = gpui::size(bounds.size.width, px(1.));
                let rule = Bounds::new(bar.origin, rule);
                let rule_fill = (!w.selected)
                    .then(|| {
                        settings
                            .theme
                            .face(FaceName::ModeLine)
                            .bg
                            .map(|bg| (rule, bg))
                    })
                    .flatten();
                Painted {
                    bounds,
                    selected: w.selected,
                    text_x,
                    rows,
                    mode_line_fill: bar_fill.into_iter().chain(rule_fill).collect(),
                    mode_line: group(&w.mode_line.left),
                    mode_line_right: group(&w.mode_line.right),
                    cells,
                    cursor,
                    cursor_char,
                    hollow,
                }
            });
            let painted = painted.collect();
            if let Some(debug) = &mut editor.debug {
                debug.shape = start.elapsed();
            }
            painted
        })
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut (),
        painted: &mut Vec<Painted>,
        window: &mut Window,
        cx: &mut App,
    ) {
        let editor = self.editor.read(cx);
        let focus = editor.focus.clone();
        let theme = editor.core.settings().theme.clone();
        let line_height = px(editor.core.settings().line_height());
        let mode_line_height = px(editor.core.settings().mode_line_height());
        window.handle_input(
            &focus,
            ElementInputHandler::new(bounds, self.editor.clone()),
            cx,
        );
        // Windows tile the area, so the gaps between them are the only divider left showing.
        if let Some(divider) = theme.face(FaceName::Divider).bg {
            window.paint_quad(fill(bounds, rgb(divider)));
        }
        for p in painted.iter() {
            window.paint_quad(fill(p.bounds, rgb(theme.background())));
            let mask = ContentMask { bounds: p.bounds };
            window.with_content_mask(Some(mask), |window| {
                let width = p.bounds.size.width;
                for row in &p.rows {
                    let origin = p.bounds.origin + point(px(0.), row.top);
                    for decoration in &row.decorations {
                        let (width, color) = match *decoration {
                            Decoration::Background(name) => (width, theme.face(name).bg),
                            Decoration::Rule(name) => (px(2.), theme.face(name).fg),
                        };
                        if let Some(color) = color {
                            let bounds = Bounds::new(origin, size(width, row.height));
                            window.paint_quad(fill(bounds, rgb(color)));
                        }
                    }
                    if let Some((x, number)) = &row.number {
                        let at = origin + point(*x, px(0.));
                        number.paint_background(at, row.height, window, cx).ok();
                        number.paint(at, row.height, window, cx).ok();
                    }
                    let origin = origin + point(p.text_x, px(0.));
                    row.text
                        .paint_background(origin, row.height, window, cx)
                        .ok();
                    row.text.paint(origin, row.height, window, cx).ok();
                }
                for c in &p.cells {
                    if let Some((bounds, color)) = c.background {
                        window.paint_quad(fill(bounds, rgb(color)));
                    }
                }
                for c in &p.cells {
                    c.text.paint(c.origin, line_height, window, cx).ok();
                }
                if let Some(cursor) = p.cursor.filter(|_| p.hollow) {
                    window.paint_quad(outline(cursor, rgb(theme.cursor()), BorderStyle::Solid));
                } else if let Some(cursor) = p.cursor {
                    window.paint_quad(fill(cursor, rgb(theme.cursor())));
                    if let Some(under) = &p.cursor_char {
                        under
                            .paint(cursor.origin, cursor.size.height, window, cx)
                            .ok();
                    }
                }
                for (bounds, color) in &p.mode_line_fill {
                    window.paint_quad(fill(*bounds, rgb(*color)));
                }
                let x = p.bounds.left() + px(TEXT_INSET);
                let origin = point(x, p.bounds.bottom() - mode_line_height);
                p.mode_line.paint(origin, mode_line_height, window, cx).ok();
                let right = point(p.mode_line_right_x(), origin.y);
                (p.mode_line_right)
                    .paint(right, mode_line_height, window, cx)
                    .ok();
            });
        }
        let painted = std::mem::take(painted);
        self.editor.update(cx, |editor, _| {
            if let Some(benchmark) = &mut editor.benchmark {
                let text = painted
                    .iter()
                    .find(|p| p.selected)
                    .and_then(|p| p.rows.first())
                    .map(|r| r.text.text.as_ref());
                benchmark.painted(Instant::now(), text);
            }
            editor.painted = painted;
        });
    }
}

/// The block cursor at byte `byte` of `line`: its x, its width and the char it covers, which is
/// empty at the end of the line, where the block is one `cell` wide.
pub(crate) fn block(line: &ShapedLine, byte: usize, cell: Pixels) -> (Pixels, Pixels, &str) {
    let x = line.x_for_index(byte);
    let under =
        (line.text[byte..].chars().next()).map_or("", |c| &line.text[byte..byte + c.len_utf8()]);
    let width = match under.is_empty() {
        true => cell,
        false => line.x_for_index(byte + under.len()) - x,
    };
    (x, width, under)
}

/// The char a terminal's block cursor at `row`, `col` covers, if any is drawn there.
fn under_cursor(terminal: &Terminal, row: u16, col: u16) -> Option<String> {
    let run =
        (terminal.rows[row as usize].iter()).find(|r| (r.col..r.col + r.cells).contains(&col))?;
    // A wide char runs alone and the cursor sits on its first cell; combining marks only end a run.
    Some(run.text.chars().nth((col - run.col) as usize)?.to_string())
}

/// `font` in bold weight and italic style as asked.
fn styled(font: &Font, bold: bool, italic: bool) -> Font {
    Font {
        weight: if bold { FontWeight::BOLD } else { font.weight },
        style: if italic {
            FontStyle::Italic
        } else {
            font.style
        },
        ..font.clone()
    }
}

/// `rect` from the frame display, placed at `origin`.
fn to_bounds(rect: Rect, origin: Point<Pixels>) -> Bounds<Pixels> {
    let at = point(px(rect.x), px(rect.y));
    Bounds::new(origin + at, size(px(rect.width), px(rect.height)))
}

pub(crate) fn text_run(
    len: usize,
    face: Face,
    underline: bool,
    font: &Font,
    theme: &Theme,
) -> TextRun {
    TextRun {
        len,
        font: styled(font, face.bold, face.italic),
        color: rgb(face.fg.unwrap_or(theme.foreground())).into(),
        background_color: face.bg.map(|bg| rgb(bg).into()),
        underline: (underline || face.underline).then(|| UnderlineStyle {
            thickness: px(1.),
            ..Default::default()
        }),
        strikethrough: None,
    }
}

pub(crate) fn completion_runs(
    len: usize,
    highlights: &[std::ops::Range<usize>],
    font: &Font,
    theme: &Theme,
) -> Vec<TextRun> {
    let mut runs = Vec::new();
    let mut start = 0;
    for range in highlights {
        if start < range.start {
            runs.push(text_run(
                range.start - start,
                Face::default(),
                false,
                font,
                theme,
            ));
        }
        runs.push(text_run(
            range.len(),
            theme.face(FaceName::CompletionMatch),
            false,
            font,
            theme,
        ));
        start = range.end;
    }
    if start < len {
        runs.push(text_run(len - start, Face::default(), false, font, theme));
    }
    runs
}

/// The echo line at a prompt: the match count, the prompt, the minibuffer's line and any `message` in brackets,
/// with the byte offset of point in it.
pub(crate) fn echo_line(mb: &aiditor_core::display::Minibuffer, message: &str) -> (String, usize) {
    let spans = mb.display.lines.iter().flat_map(|l| &l.spans);
    let text: String = spans.map(|s| s.text.as_str()).collect();
    let col = mb.display.cursor.map_or(usize::MAX, |(_, col)| col);
    let byte = text.char_indices().nth(col).map_or(text.len(), |(b, _)| b);
    // The echo line is shaped as one line; a prompt or message may name a buffer with newlines.
    let prompt = format!("{}{}", mb.count, mb.prompt.replace('\n', "^J"));
    let message = match message.is_empty() {
        true => String::new(),
        false => format!(" [{}]", message.replace('\n', "^J")),
    };
    (format!("{prompt}{text}{message}"), prompt.len() + byte)
}

/// `line`'s text with `marked` spliced in at char column `at`, one run per span, and the byte
/// offset the cursor goes at: after the marked text.
fn line_runs(
    line: &Line,
    at: Option<usize>,
    marked: &str,
    font: &Font,
    theme: &Theme,
) -> (String, Vec<TextRun>, usize) {
    let mut pieces: Vec<(&str, Face, bool)> = Vec::new();
    let mut col = at;
    for span in &line.spans {
        let chars = span.text.chars().count();
        match col {
            Some(c) if c <= chars => {
                let byte = span
                    .text
                    .char_indices()
                    .nth(c)
                    .map_or(span.text.len(), |(b, _)| b);
                pieces.push((&span.text[..byte], span.face, false));
                pieces.push((marked, Face::default(), true));
                pieces.push((&span.text[byte..], span.face, false));
                col = None;
            }
            _ => {
                col = col.map(|c| c - chars);
                pieces.push((&span.text, span.face, false));
            }
        }
    }
    if col.is_some() {
        pieces.push((marked, Face::default(), true));
    }
    let (mut text, mut runs, mut cursor) = (String::new(), Vec::new(), 0);
    for (piece, face, is_marked) in pieces.into_iter().filter(|p| !p.0.is_empty() || p.2) {
        text += piece;
        if is_marked {
            cursor = text.len();
        }
        if !piece.is_empty() {
            runs.push(text_run(piece.len(), face, is_marked, font, theme));
        }
    }
    (text, runs, cursor)
}
