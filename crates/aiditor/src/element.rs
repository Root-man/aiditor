use std::time::Instant;

use aiditor_core::Msg;
use aiditor_core::buffer::{Decoration, Face};
use aiditor_core::display::{FrameWindow, Line, Rect};
use aiditor_core::settings::Theme;
use aiditor_core::window::WindowId;
use gpui::{
    App, Bounds, ContentMask, Element, ElementId, ElementInputHandler, Entity, Font,
    GlobalElementId, InspectorElementId, LayoutId, Pixels, Point, ShapedLine, Style, TextRun,
    UnderlineStyle, Window, fill, point, prelude::*, px, rgb, size,
};

use crate::editor::Editor;

/// A window as painted: where, its rows and mode line laid out, and the cursor.
pub(crate) struct Painted {
    pub(crate) id: WindowId,
    pub(crate) bounds: Bounds<Pixels>,
    selected: bool,
    pub(crate) rows: Vec<Row>,
    mode_line: ShapedLine,
    pub(crate) cursor: Option<Bounds<Pixels>>,
}

pub(crate) struct Row {
    /// The buffer line it starts on.
    pub(crate) number: usize,
    /// Its top, from the window's.
    pub(crate) top: Pixels,
    pub(crate) height: Pixels,
    decorations: Vec<Decoration>,
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
            let painted = std::mem::take(&mut self.windows).into_iter().map(|w| {
                let bounds = to_bounds(w.rect, bounds.origin);
                let mut top = px(0.);
                let mut cursor = None;
                let rows = (w.display.lines.iter().enumerate()).map(|(i, line)| {
                    let height = px(line.height);
                    let at = w.display.cursor.filter(|&(l, _)| l == i && w.selected);
                    let marked = at.and(editor.marked.as_deref()).unwrap_or_default();
                    let (text, runs, byte) =
                        line_runs(line, at.map(|(_, col)| col), marked, &font, &settings.theme);
                    let text = shape(text, &runs);
                    if at.is_some() {
                        let x = text.x_for_index(byte);
                        let origin = bounds.origin + point(x, top);
                        cursor = Some(Bounds::new(origin, gpui::size(px(2.), height)));
                    }
                    let row = Row {
                        number: line.number,
                        top,
                        height,
                        decorations: line.decorations.clone(),
                        text,
                    };
                    top += height;
                    row
                });
                let rows = rows.collect();
                let run = text_run(
                    w.display.mode_line.len(),
                    Face::default(),
                    false,
                    &font,
                    &settings.theme,
                );
                Painted {
                    id: w.id,
                    bounds,
                    selected: w.selected,
                    rows,
                    mode_line: shape(w.display.mode_line, &[run]),
                    cursor,
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
        window.handle_input(
            &focus,
            ElementInputHandler::new(bounds, self.editor.clone()),
            cx,
        );
        for p in painted.iter() {
            let mask = ContentMask { bounds: p.bounds };
            window.with_content_mask(Some(mask), |window| {
                let width = p.bounds.size.width;
                for row in &p.rows {
                    let origin = p.bounds.origin + point(px(0.), row.top);
                    for decoration in &row.decorations {
                        let (width, color) = match *decoration {
                            Decoration::Background(color) => (width, color),
                            Decoration::Rule(color) => (px(2.), color),
                        };
                        let bounds = Bounds::new(origin, size(width, row.height));
                        window.paint_quad(fill(bounds, rgb(color)));
                    }
                    row.text
                        .paint_background(origin, row.height, window, cx)
                        .ok();
                    row.text.paint(origin, row.height, window, cx).ok();
                }
                if let Some(cursor) = p.cursor {
                    window.paint_quad(fill(cursor, rgb(theme.cursor)));
                }
                let origin = point(p.bounds.left(), p.bounds.bottom() - line_height);
                let color = match p.selected {
                    true => theme.mode_line,
                    false => theme.mode_line_inactive,
                };
                let bounds = Bounds::new(origin, size(width, line_height));
                window.paint_quad(fill(bounds, rgb(color)));
                p.mode_line.paint(origin, line_height, window, cx).ok();
            });
        }
        let painted = std::mem::take(painted);
        self.editor.update(cx, |editor, _| editor.painted = painted);
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
        font: font.clone(),
        color: rgb(face.fg.unwrap_or(theme.foreground)).into(),
        background_color: face.bg.map(|bg| rgb(bg).into()),
        underline: underline.then(|| UnderlineStyle {
            thickness: px(1.),
            ..Default::default()
        }),
        strikethrough: None,
    }
}

/// The echo line at a prompt: the prompt, the minibuffer's line and any `message` in brackets,
/// with the byte offset of point in it.
pub(crate) fn echo_line(mb: &aiditor_core::display::Minibuffer, message: &str) -> (String, usize) {
    let spans = mb.display.lines.iter().flat_map(|l| &l.spans);
    let text: String = spans.map(|s| s.text.as_str()).collect();
    let col = mb.display.cursor.map_or(usize::MAX, |(_, col)| col);
    let byte = text.char_indices().nth(col).map_or(text.len(), |(b, _)| b);
    // The echo line is shaped as one line; a prompt or message may name a buffer with newlines.
    let prompt = mb.prompt.replace('\n', "^J");
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
