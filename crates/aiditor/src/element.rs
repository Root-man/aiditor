use std::ops::Range;
use std::time::Instant;

use aiditor_core::Msg;
use aiditor_core::buffer::{Decoration, Face, Icon};
use aiditor_core::display::{
    CursorShape, FrameWindow, GUTTER_RIGHT, Line, PANE_PADDING, Rect, Span, TEXT_INSET, Terminal,
};
use aiditor_core::settings::{FaceName, Theme};
use gpui::{
    App, BorderStyle, Bounds, ContentMask, Corners, Edges, Element, ElementId, ElementInputHandler,
    Entity, Font, FontStyle, FontWeight, GlobalElementId, Hsla, InspectorElementId, LayoutId,
    Pixels, Point, ShapedLine, StrikethroughStyle, Style, TextRun, TransformationMatrix,
    UnderlineStyle, Window, fill, outline, point, prelude::*, px, quad, rgb, size,
    transparent_black,
};

use crate::editor::Editor;

/// A window as painted: where, its rows and mode line laid out, and the cursor.
pub(crate) struct Painted {
    pub(crate) bounds: Bounds<Pixels>,
    pub(crate) selected: bool,
    /// Where its text starts, past the gutter, from the window's left.
    pub(crate) text_x: Pixels,
    /// Where its text stops, from the window's right.
    text_right: Pixels,
    /// A char cell's width, where a table's rules stand.
    cell: Pixels,
    pub(crate) rows: Vec<Row>,
    /// The rule above a docked prompt pane.
    pub(crate) rule: Option<Bounds<Pixels>>,
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
    /// The height its text is set in: less than `height` on a `Small` line, so it sits at the top.
    text_height: Pixels,
    decorations: Vec<Decoration>,
    /// Where an `HRule` decoration shows, as x from the window's left.
    gaps: Vec<Range<Pixels>>,
    /// How far right of the text's start its text and background start.
    indent: Pixels,
    /// Its label in the gutter, if the window has one, and the label's x from the window's left.
    pub(crate) number: Option<(Pixels, ShapedLine)>,
    pub(crate) text: ShapedLine,
    pub(crate) boxes: Vec<Boxed>,
    pub(crate) icons: Vec<RowIcon>,
}

/// A boxed run on a row: its x bounds from the text's start, its background and box colours, and
/// whether it shares the outline with the run on its left or right.
#[derive(Debug, PartialEq)]
pub(crate) struct Boxed {
    pub(crate) x: Range<Pixels>,
    pub(crate) bg: Option<u32>,
    pub(crate) color: u32,
    pub(crate) open_left: bool,
    pub(crate) open_right: bool,
}

/// An icon on a row: its two cells' x bounds from the text's start, and its run's foreground.
#[derive(Debug, PartialEq)]
pub(crate) struct RowIcon {
    pub(crate) icon: Icon,
    pub(crate) x: Range<Pixels>,
    pub(crate) color: Hsla,
}

/// A `Small` line's font size against the frame's, frame 6's 11px timings beside 15px text.
const SMALL: f32 = 11. / 15.;

/// A boxed run's corners, per the design's chip.
const BOX_RADIUS: f32 = 5.;

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
                let text_x = px(w.gutter.as_ref().map_or(w.inset.0, |g| g.width));
                let mut top = px(0.);
                let mut cursor = None;
                let mut cursor_char = None;
                // A docked prompt pane's rows follow the transcript's, ending the padding above
                // the mode line.
                let display = &w.display;
                let pane = display.prompt.as_ref();
                let pane_rows: f32 = pane.map_or(0., |p| p.lines.iter().map(|l| l.height).sum());
                let mode_line_height = px(settings.mode_line_height());
                let pane_top =
                    px(w.rect.height) - mode_line_height - px(PANE_PADDING) - px(pane_rows);
                let pane_rule = pane.map(|_| {
                    let top = bounds.origin + point(px(0.), pane_top - px(PANE_PADDING + 1.));
                    Bounds::new(top, gpui::size(bounds.size.width, px(1.)))
                });
                let transcript = (display.lines.iter().enumerate()).map(|(i, line)| {
                    let at = display.cursor.filter(|&(l, _)| l == i);
                    (
                        Some(i),
                        line,
                        at.map(|(_, col)| (col, display.cursor_width)),
                    )
                });
                let docked = pane.into_iter().flat_map(|p| {
                    (p.lines.iter().enumerate()).map(move |(i, line)| {
                        let at = p.cursor.filter(|&(l, _)| l == i);
                        (None, line, at.map(|(_, col)| (col, p.cursor_width)))
                    })
                });
                let mut in_pane = false;
                let rows = transcript.chain(docked).map(|(i, line, at)| {
                    if i.is_none() && !in_pane {
                        (in_pane, top) = (true, pane_top);
                    }
                    let height = px(line.height);
                    let indent = cell * line.indent() as f32;
                    let (at, chars) = match at.filter(|_| w.selected) {
                        Some((col, chars)) => (Some((0, col)), chars),
                        None => (None, 1),
                    };
                    let marked = at.and(editor.marked.as_deref()).unwrap_or_default();
                    let (text, runs, byte, boxes) =
                        line_runs(line, at.map(|(_, col)| col), marked, &font, &settings.theme);
                    let small = line.decorations.contains(&Decoration::Small);
                    let text = match small {
                        true => (window.text_system()).shape_line(
                            text.into(),
                            font_size * SMALL,
                            &runs,
                            None,
                        ),
                        false => shape(text, &runs),
                    };
                    let text_height = match small {
                        true => line_height * SMALL,
                        false => height,
                    };
                    let boxes = boxed(&text, boxes);
                    let icons = icons(line, at.map(|(_, col)| col), marked, &text, &runs);
                    if at.is_some() {
                        let (x, width, under) = block(&text, byte, chars, cell);
                        let origin = bounds.origin + point(text_x + indent + x, top);
                        cursor = Some(Bounds::new(origin, gpui::size(width, height)));
                        let face = Face {
                            fg: settings.theme.face(FaceName::Cursor).fg,
                            ..Face::default()
                        };
                        let run = text_run(under.len(), face, false, &font, &settings.theme);
                        cursor_char = Some(shape(under.to_string(), &[run]));
                    }
                    let number = w.gutter.as_ref().zip(i).map(|(g, i)| {
                        let (label, face) = &g.rows[i];
                        let run = text_run(label.len(), *face, false, &font, &settings.theme);
                        let label = shape(label.clone(), &[run]);
                        (text_x - px(GUTTER_RIGHT) - label.width, label)
                    });
                    let start = text_x + indent;
                    let gaps = match line
                        .decorations
                        .iter()
                        .any(|d| matches!(d, Decoration::HRule(_)))
                    {
                        true => rule_gaps(
                            &text.text,
                            |i| start + text.x_for_index(i),
                            bounds.size.width,
                            cell,
                        ),
                        false => Vec::new(),
                    };
                    let row = Row {
                        top,
                        height,
                        text_height,
                        gaps,
                        decorations: line.decorations.clone(),
                        indent,
                        number,
                        text,
                        boxes,
                        icons,
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
                    text_right: px(w.inset.1),
                    cell,
                    rows,
                    rule: pane_rule,
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
        let font_size = px(editor.core.settings().font_size);
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
                        // An indented line's background runs from its text to the right inset.
                        let left = match row.indent > px(0.) {
                            true => p.text_x + row.indent,
                            false => px(0.),
                        };
                        let (h, one) = (row.height, px(1.));
                        // Each quad is x, y, width and height from the row's left top.
                        let (color, quads) = match *decoration {
                            Decoration::Background(name) if left > px(0.) => {
                                let w = width - left - p.text_right;
                                (theme.face(name).bg, vec![(left, px(0.), w, h)])
                            }
                            Decoration::Background(name) => {
                                (theme.face(name).bg, vec![(left, px(0.), width, h)])
                            }
                            Decoration::Rule(name) => {
                                (theme.face(name).fg, vec![(px(0.), px(0.), px(2.), h)])
                            }
                            Decoration::Bar(name) => {
                                (theme.face(name).fg, vec![(px(0.), px(0.), px(3.), h)])
                            }
                            Decoration::Indent(_) | Decoration::Small => (None, Vec::new()),
                            Decoration::Border { top, face } => {
                                let y = if top { px(0.) } else { h - one };
                                (theme.face(face).fg, vec![(px(0.), y, width, one)])
                            }
                            Decoration::Table { ref edges, first } => {
                                let text = p.text_x..width - p.text_right;
                                let rules = table_rules(edges, first, text, p.cell, h);
                                (theme.face(FaceName::Separator).fg, rules)
                            }
                            Decoration::HRule(face) => {
                                let y = (h / 2.).floor();
                                let gaps =
                                    row.gaps.iter().map(|g| (g.start, y, g.end - g.start, one));
                                (theme.face(face).fg, gaps.collect())
                            }
                        };
                        for (x, y, w, h) in quads {
                            if let Some(color) = color {
                                let at = origin + point(x, y);
                                window.paint_quad(fill(Bounds::new(at, size(w, h)), rgb(color)));
                            }
                        }
                    }
                    if let Some((x, number)) = &row.number {
                        let at = origin + point(*x, px(0.));
                        number.paint_background(at, row.height, window, cx).ok();
                        number.paint(at, row.height, window, cx).ok();
                    }
                    let origin = origin + point(p.text_x + row.indent, px(0.));
                    row.text
                        .paint_background(origin, row.height, window, cx)
                        .ok();
                    for b in &row.boxes {
                        window.paint_quad(box_quad(b, origin, row.height));
                    }
                    row.text.paint(origin, row.text_height, window, cx).ok();
                    for i in &row.icons {
                        let bounds = icon_bounds(i, origin, row.height, font_size);
                        let path = crate::icons::path(i.icon).into();
                        let unit = TransformationMatrix::unit();
                        window.paint_svg(bounds, path, unit, i.color, cx).ok();
                    }
                }
                if let (Some(rule), Some(color)) = (p.rule, theme.face(FaceName::Separator).fg) {
                    window.paint_quad(fill(rule, rgb(color)));
                }
                for c in &p.cells {
                    if let Some((bounds, color)) = c.background {
                        window.paint_quad(fill(bounds, rgb(color)));
                    }
                }
                for c in &p.cells {
                    c.text.paint(c.origin, line_height, window, cx).ok();
                }
                // After the rows, so a block cursor covers text and icons alike.
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
pub(crate) fn block(
    line: &ShapedLine,
    byte: usize,
    chars: usize,
    cell: Pixels,
) -> (Pixels, Pixels, &str) {
    let x = line.x_for_index(byte);
    let rest = &line.text[byte..];
    let end = rest
        .char_indices()
        .nth(chars)
        .map_or(rest.len(), |(b, _)| b);
    let under = &rest[..end];
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
        strikethrough: face.strike.then(|| StrikethroughStyle {
            thickness: px(1.),
            ..Default::default()
        }),
    }
}

pub(crate) fn completion_runs(
    len: usize,
    faces: &[(std::ops::Range<usize>, Face)],
    face: Face,
    selected_bg: Option<u32>,
    font: &Font,
    theme: &Theme,
) -> Vec<TextRun> {
    let mut runs = Vec::new();
    let mut start = 0;
    for (range, matched) in faces {
        if start < range.start {
            runs.push(text_run(range.start - start, face, false, font, theme));
        }
        runs.push(text_run(
            range.len(),
            Face {
                bg: selected_bg.or(matched.bg),
                ..*matched
            },
            false,
            font,
            theme,
        ));
        start = range.end;
    }
    if start < len {
        runs.push(text_run(len - start, face, false, font, theme));
    }
    runs
}

pub(crate) fn key_hint_runs(
    text: &str,
    key_len: usize,
    font: &Font,
    theme: &Theme,
) -> Vec<TextRun> {
    vec![
        text_run(
            key_len,
            Face {
                fg: theme.face(FaceName::WorkspaceCurrent).fg,
                bold: true,
                ..Face::default()
            },
            false,
            font,
            theme,
        ),
        text_run(
            text.len() - key_len,
            theme.face(FaceName::ModeLineDim),
            false,
            font,
            theme,
        ),
    ]
}

pub(crate) fn grep_runs(text: &str, font: &Font, theme: &Theme) -> Vec<TextRun> {
    let number = text.find(':').expect("grep line number");
    vec![
        text_run(
            number,
            theme.face(FaceName::GrepLineNumber),
            false,
            font,
            theme,
        ),
        text_run(text.len() - number, Face::default(), false, font, theme),
    ]
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

/// The boxed `pieces` of `text`, laid out. Touching pieces in one box colour share its outline.
pub(crate) fn boxed(text: &ShapedLine, pieces: Vec<(Range<usize>, Face)>) -> Vec<Boxed> {
    let shared = |a: &(Range<usize>, Face), b: &(Range<usize>, Face)| {
        a.0.end == b.0.start && a.1.boxed == b.1.boxed
    };
    (pieces.iter().enumerate())
        .filter_map(|(i, piece)| {
            let (range, face) = piece;
            Some(Boxed {
                x: text.x_for_index(range.start)..text.x_for_index(range.end),
                bg: face.bg,
                color: face.boxed?,
                open_left: i > 0 && shared(&pieces[i - 1], piece),
                open_right: pieces.get(i + 1).is_some_and(|next| shared(piece, next)),
            })
        })
        .collect()
}

/// `b` on a row of `height` whose text starts at `origin`, 1px inside the row at top and bottom,
/// with no side or corners where it shares the outline.
pub(crate) fn box_quad(b: &Boxed, origin: Point<Pixels>, height: Pixels) -> gpui::PaintQuad {
    let top_left = origin + point(b.x.start, px(1.));
    let bottom_right = origin + point(b.x.end, height - px(1.));
    let side = |open| px(if open { 0. } else { 1. });
    let corner = |open| px(if open { 0. } else { BOX_RADIUS });
    quad(
        Bounds::from_corners(top_left, bottom_right),
        Corners {
            top_left: corner(b.open_left),
            top_right: corner(b.open_right),
            bottom_right: corner(b.open_right),
            bottom_left: corner(b.open_left),
        },
        b.bg.map_or(transparent_black(), |bg| rgb(bg).into()),
        Edges {
            top: px(1.),
            right: side(b.open_right),
            bottom: px(1.),
            left: side(b.open_left),
        },
        rgb(b.color),
        BorderStyle::Solid,
    )
}

/// `line`'s icons on its shaped `text`, which has `marked` spliced in at char column `at`, in the
/// colours of `text`'s `runs`.
pub(crate) fn icons(
    line: &Line,
    at: Option<usize>,
    marked: &str,
    text: &ShapedLine,
    runs: &[TextRun],
) -> Vec<RowIcon> {
    let spliced = marked.chars().count();
    (line.icons.iter())
        .map(|&(col, icon)| {
            let col = col
                + if at.is_some_and(|a| a <= col) {
                    spliced
                } else {
                    0
                };
            let byte = text
                .text
                .char_indices()
                .nth(col)
                .map_or(text.len(), |(b, _)| b);
            let mut end = 0;
            let run = runs.iter().find(|r| {
                end += r.len;
                end > byte
            });
            RowIcon {
                icon,
                x: text.x_for_index(byte)..text.x_for_index(byte + 2),
                color: run.expect("an icon's spaces are in the text").color,
            }
        })
        .collect()
}

/// Where `i` is painted on a row of `height` whose text starts at `origin`: a square centred on
/// its cells, as big as the font or the cells' width, whichever is less.
pub(crate) fn icon_bounds(
    i: &RowIcon,
    origin: Point<Pixels>,
    height: Pixels,
    font_size: Pixels,
) -> Bounds<Pixels> {
    let width = i.x.end - i.x.start;
    let side = font_size.min(width);
    let at = point(i.x.start + (width - side) / 2., (height - side) / 2.);
    Bounds::new(origin + at, size(side, side))
}

/// `line`'s text with `marked` spliced in at char column `at`, one run per span, the byte offset
/// the cursor goes at, after the marked text, and the boxed pieces' byte ranges, which take their
/// background from the box instead of their run.
#[allow(clippy::type_complexity)]
pub(crate) fn line_runs(
    line: &Line,
    at: Option<usize>,
    marked: &str,
    font: &Font,
    theme: &Theme,
) -> (String, Vec<TextRun>, usize, Vec<(Range<usize>, Face)>) {
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
    let (mut text, mut runs, mut cursor, mut boxes) = (String::new(), Vec::new(), 0, Vec::new());
    for (piece, mut face, is_marked) in pieces.into_iter().filter(|p| !p.0.is_empty() || p.2) {
        text += piece;
        if is_marked {
            cursor = text.len();
        }
        if !piece.is_empty() {
            if face.boxed.is_some() {
                boxes.push((text.len() - piece.len()..text.len(), face));
                face.bg = None;
            }
            runs.push(text_run(piece.len(), face, is_marked, font, theme));
        }
    }
    (text, runs, cursor, boxes)
}

/// The stretches of a row with no glyph, a cell clear of the glyphs beside them, as x from the
/// window's left: from its left edge, between glyphs and to `right`. `x` is a byte's x.
fn rule_gaps(
    text: &str,
    x: impl Fn(usize) -> Pixels,
    right: Pixels,
    cell: Pixels,
) -> Vec<Range<Pixels>> {
    let (mut gaps, mut from) = (Vec::new(), Some(px(0.)));
    for (i, c) in text.char_indices() {
        match (c == ' ', from) {
            (true, None) => from = Some(x(i) + cell),
            (false, Some(start)) => {
                gaps.push(start..x(i) - cell);
                from = None;
            }
            _ => {}
        }
    }
    gaps.push(from.unwrap_or_else(|| x(text.len()) + cell)..right);
    gaps.retain(|g| g.end > g.start);
    gaps
}

/// A table row's 1 px rules as x, y, width and height from the row's left top: one down the row
/// at each char column of `edges` within the `text` area's x range, one along its bottom from
/// its first edge to its last and, on its `first` row, one along its top, cut to the area.
fn table_rules(
    edges: &[isize],
    first: bool,
    text: Range<Pixels>,
    cell: Pixels,
    h: Pixels,
) -> Vec<(Pixels, Pixels, Pixels, Pixels)> {
    let one = px(1.);
    let x = |e: isize| text.start + cell * e as f32;
    let (Some(&left), Some(&right)) = (edges.first(), edges.last()) else {
        return Vec::new();
    };
    let (from, to) = (x(left).max(text.start), x(right).min(text.end));
    let mut rules: Vec<_> = (edges.iter().map(|&e| x(e)))
        // A table ending at the area's edge keeps its right rule, in the inset beside the text.
        .filter(|x| text.contains(x) || *x == text.end)
        .map(|x| (x, px(0.), one, h))
        .collect();
    if from < to {
        rules.push((from, h - one, to - from, one));
        if first {
            rules.push((from, px(0.), to - from, one));
        }
    }
    rules
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_rules_stand_at_cell_edges_cut_to_the_text() {
        let (one, h) = (px(1.), px(20.));
        let text = px(12.)..px(400.);
        assert_eq!(
            table_rules(&[0, 3, 7], true, text.clone(), px(10.), h),
            [
                (px(12.), px(0.), one, h),
                (px(42.), px(0.), one, h),
                (px(82.), px(0.), one, h),
                (px(12.), h - one, px(70.), one),
                (px(12.), px(0.), px(70.), one),
            ]
        );
        // Scrolled sideways and wider than the window: only the edges inside are drawn.
        assert_eq!(
            table_rules(&[-5, 3, 50], false, text, px(10.), h),
            [(px(42.), px(0.), one, h), (px(12.), h - one, px(388.), one),]
        );
    }

    #[test]
    fn a_rule_fills_the_gaps_a_cell_clear_of_the_text() {
        // Ten-pixel cells, the text starting two cells in.
        let x = |i: usize| px(20. + 10. * i as f32);
        let gaps = rule_gaps("Worked 2m 05s     13:58:12", x, px(400.), px(10.));
        assert_eq!(
            gaps,
            [px(0.)..px(10.), px(160.)..px(190.), px(290.)..px(400.)]
        );
        assert_eq!(rule_gaps("", x, px(100.), px(10.)), [px(0.)..px(100.)]);
    }
}
