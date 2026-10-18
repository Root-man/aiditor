use std::collections::VecDeque;
use std::ffi::OsString;
use std::io::ErrorKind;
use std::ops::Range;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use aiditor_core::buffer::{Decoration, Face};
use aiditor_core::display::{Line, Rect};
use aiditor_core::key::{Key, KeyCode};
use aiditor_core::settings::Theme;
use aiditor_core::window::WindowId;
use aiditor_core::{Core, Effect, Msg};
use aiditor_services::{files, recovery};
use gpui::{
    App, Application, Bounds, ClipboardItem, ContentMask, Context, Element, ElementId,
    ElementInputHandler, Entity, EntityInputHandler, FocusHandle, Font, GlobalElementId,
    InspectorElementId, KeyDownEvent, Keystroke, LayoutId, MouseButton, MouseDownEvent, Pixels,
    Point, ScrollWheelEvent, ShapedLine, Style, Task, TextRun, UTF16Selection, UnderlineStyle,
    Window, WindowOptions, div, fill, font, point, prelude::*, px, rgb, size,
};
use tokio::sync::mpsc;
use tracing_subscriber::prelude::*;
use tracing_subscriber::registry::LookupSpan;
use tracing_subscriber::{EnvFilter, Layer, filter::LevelFilter, fmt};

const FONT: &str = if cfg!(target_os = "macos") {
    "Menlo"
} else {
    "Noto Sans Mono"
};

struct Editor {
    core: Core,
    focus: FocusHandle,
    /// Services send their results to the core on this.
    tx: mpsc::Sender<Msg>,
    /// Wakes the core at its next deadline; replacing it cancels the old one.
    timer: Option<Task<()>>,
    /// Where the user's settings and keymap files are, if known.
    config_dir: Option<PathBuf>,
    /// The recovery service, when there is a data dir.
    recovery: Option<(std::sync::mpsc::Sender<Effect>, std::thread::JoinHandle<()>)>,
    /// Set by `--debug`: the stats line drawn at the top.
    debug: Option<Debug>,
    /// The input method's marked text, drawn at point but not in the buffer.
    marked: Option<String>,
    /// The windows as last painted, for clicks, the wheel and the input method.
    painted: Vec<Painted>,
    /// The windows area's size and cell width last sent to the core.
    reported: (Pixels, Pixels, Pixels),
    /// Wheel movement not yet scrolled as a whole line, positive down the buffer.
    scroll_px: Pixels,
}

#[derive(Default)]
struct Debug {
    /// When the last frames were rendered, for the frame rate.
    frames: VecDeque<Instant>,
    /// How long the core took to handle the last message.
    handle: Duration,
    /// How long building the last frame's elements took.
    render: Duration,
    /// How long laying out the last frame's lines took.
    shape: Duration,
    /// How long the last heartbeat took from a Tokio task to the UI thread.
    tick: Duration,
}

impl Editor {
    /// `channel` is the services channel, made first so `tracing` can log into it.
    fn new(
        core: Core,
        args: Args,
        (tx, mut rx): (mpsc::Sender<Msg>, mpsc::Receiver<Msg>),
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        // Closing the window quits as `C-x C-c` does, so unsaved buffers are offered first.
        let this = cx.weak_entity();
        window.on_window_should_close(cx, move |_, cx| {
            _ = this.update(cx, |editor, cx| editor.handle(Msg::Quit, cx));
            false
        });
        cx.spawn(async move |this, cx| {
            while let Some(msg) = rx.recv().await {
                if this
                    .update(cx, |editor, cx| editor.handle(msg, cx))
                    .is_err()
                {
                    break;
                }
            }
        })
        .detach();
        if args.debug {
            spawn_heartbeat(cx);
        }
        Editor {
            core,
            focus: cx.focus_handle(),
            tx,
            timer: None,
            config_dir: args.config_dir,
            recovery: args.recovery_dir.map(recovery::spawn),
            debug: args.debug.then(Debug::default),
            marked: None,
            painted: Vec::new(),
            reported: Default::default(),
            scroll_px: px(0.),
        }
    }

    /// Every message reaches the core here, so the timer is always re-armed.
    fn handle(&mut self, msg: Msg, cx: &mut Context<Self>) {
        let start = Instant::now();
        let effects = self.core.handle(start, msg);
        if let Some(debug) = &mut self.debug {
            debug.handle = start.elapsed();
        }
        for effect in effects {
            match effect {
                Effect::Save {
                    buffer,
                    tick,
                    path,
                    bytes,
                    hash,
                } => {
                    let tx = self.tx.clone();
                    tokio::task::spawn_blocking(move || {
                        let result = files::write(&path, &bytes, hash);
                        match &result {
                            Ok(_) => tracing::info!(path = %path.display(), "saved"),
                            Err(err) => tracing::error!(%err, "save failed"),
                        }
                        _ = tx.blocking_send(Msg::Saved {
                            buffer,
                            tick,
                            result,
                        });
                    });
                }
                Effect::ListDir(dir) => {
                    let tx = self.tx.clone();
                    tokio::task::spawn_blocking(move || {
                        let result =
                            files::list(&dir).map_err(|err| format!("{}: {err}", dir.display()));
                        _ = tx.blocking_send(Msg::Listed { dir, result });
                    });
                }
                Effect::Open(path) => {
                    let tx = self.tx.clone();
                    tokio::task::spawn_blocking(move || {
                        let result =
                            files::read(&path).map_err(|err| format!("{}: {err}", path.display()));
                        _ = tx.blocking_send(Msg::Opened { path, result });
                    });
                }
                Effect::SetClipboard(text) => {
                    cx.write_to_clipboard(ClipboardItem::new_string(text))
                }
                Effect::AutoSave { .. } | Effect::DeleteAutoSave(_) => {
                    if let Some((tx, _)) = &self.recovery {
                        _ = tx.send(effect);
                    }
                }
                Effect::Quit => {
                    // Lets queued recovery deletes finish before the process exits.
                    if let Some((tx, thread)) = self.recovery.take() {
                        drop(tx);
                        _ = thread.join();
                    }
                    cx.quit()
                }
                Effect::LoadSettings => {
                    let dir = self.config_dir.clone();
                    let read = |name| read_config(dir.as_deref(), name);
                    self.handle(Msg::SettingsLoaded(read("settings.toml")), cx);
                    self.handle(Msg::KeymapLoaded(read("keymap.toml")), cx);
                }
            }
        }
        self.timer = self.core.next_deadline().map(|deadline| {
            let wait = cx
                .background_executor()
                .timer(deadline.saturating_duration_since(Instant::now()));
            cx.spawn(async move |this, cx| {
                wait.await;
                this.update(cx, |editor, cx| editor.handle(Msg::Tick, cx))
                    .ok();
            })
        });
        cx.notify();
    }

    /// Command keys go to the core; the rest propagate to the platform's text input.
    fn on_key_down(&mut self, ev: &KeyDownEvent, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(key) = to_key(&ev.keystroke, self.core.settings().option_as_meta) {
            cx.stop_propagation();
            // Read only for a kill or yank: the read is a round trip to the display server on Linux.
            if self.core.wants_clipboard(key)
                && let Some(text) = cx.read_from_clipboard().and_then(|item| item.text())
            {
                self.handle(Msg::Clipboard(text), cx);
            }
            self.handle(Msg::Key(key), cx);
        }
    }

    fn on_scroll(&mut self, ev: &ScrollWheelEvent, _: &mut Window, cx: &mut Context<Self>) {
        let Some(window) = self.painted_at(ev.position).map(|p| p.id) else {
            return;
        };
        let line_height = px(self.core.settings().line_height());
        // GPUI's delta is negative when the content should move up, that is down the buffer.
        self.scroll_px -= ev.delta.pixel_delta(line_height).y;
        let lines = (self.scroll_px / line_height) as i64;
        if lines != 0 {
            self.scroll_px -= line_height * lines as f32;
            self.handle(Msg::Scroll { window, lines }, cx);
        }
    }

    fn on_click(&mut self, ev: &MouseDownEvent, _: &mut Window, cx: &mut Context<Self>) {
        let Some(p) = self.painted_at(ev.position) else {
            return;
        };
        let y = ev.position.y - p.bounds.top();
        if y >= p.bounds.size.height - px(self.core.settings().line_height()) {
            return; // the mode line
        }
        // shortcut: the column in the shown text is taken as the buffer column, wrong after a fold on the line; map through the spans when folds have producers.
        let (line, col) = match p.rows.iter().find(|row| y < row.top + row.height) {
            Some(row) => {
                let byte = row
                    .text
                    .closest_index_for_x(ev.position.x - p.bounds.left());
                (row.number, row.text.text[..byte].chars().count())
            }
            // Below the buffer's last line: its end.
            None => (usize::MAX, 0),
        };
        let window = p.id;
        self.handle(Msg::Click { window, line, col }, cx);
    }

    /// The window painted under `pos`.
    fn painted_at(&self, pos: Point<Pixels>) -> Option<&Painted> {
        self.painted.iter().find(|p| p.bounds.contains(&pos))
    }

    /// The buffer font and the width of one of its cells.
    fn font(&self, window: &Window) -> (Font, Pixels) {
        let settings = self.core.settings();
        let font = font(settings.font_family.clone().unwrap_or(FONT.into()));
        let size = px(settings.font_size);
        let text = window.text_system();
        let cell = text.ch_advance(text.resolve_font(&font), size);
        (font, cell.unwrap_or(size * 0.6))
    }

    /// Point and the marked text's length, in UTF-16 units of the text the input method sees:
    /// the buffer with the marked text at point.
    fn ime_point(&self) -> (usize, usize) {
        let point = self
            .core
            .current()
            .text()
            .char_to_utf16_cu(self.core.point());
        let marked = self.marked.as_deref().unwrap_or_default();
        (point, marked.encode_utf16().count())
    }

    /// A UTF-16 offset in the input method's text as one in the buffer; inside the marked text
    /// is at point.
    fn to_buffer(&self, offset: usize) -> usize {
        let (point, marked) = self.ime_point();
        match offset {
            o if o <= point => o,
            o if o >= point + marked => o - marked,
            _ => point,
        }
    }

    /// The buffer range an input method's range covers, or `None` if it covers none.
    fn buffer_range(&self, range: Option<Range<usize>>) -> Option<Range<usize>> {
        range
            .map(|r| self.to_buffer(r.start)..self.to_buffer(r.end))
            .filter(|r| !r.is_empty())
    }
}

impl EntityInputHandler for Editor {
    fn text_for_range(
        &mut self,
        range: Range<usize>,
        adjusted: &mut Option<Range<usize>>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<String> {
        let text = self.core.current().text();
        let (point, marked_len) = self.ime_point();
        let marked: Vec<u16> = self.marked.iter().flat_map(|m| m.encode_utf16()).collect();
        let len = text.len_utf16_cu() + marked_len;
        let (start, end) = (
            range.start.min(len),
            range.end.min(len).max(range.start.min(len)),
        );
        *adjusted = Some(start..end);
        let buffer = |a: usize, b: usize| {
            text.slice(text.utf16_cu_to_char(a)..text.utf16_cu_to_char(b))
                .to_string()
        };
        let mut out = String::new();
        if start < point {
            out += &buffer(start, end.min(point));
        }
        let (ms, me) = (
            start.clamp(point, point + marked_len),
            end.clamp(point, point + marked_len),
        );
        out += &String::from_utf16_lossy(&marked[ms - point..me - point]);
        if end > point + marked_len {
            out += &buffer(start.max(point + marked_len) - marked_len, end - marked_len);
        }
        Some(out)
    }

    fn selected_text_range(
        &mut self,
        _: bool,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        let (point, marked) = self.ime_point();
        Some(UTF16Selection {
            range: point + marked..point + marked,
            reversed: false,
        })
    }

    fn marked_text_range(&self, _: &mut Window, _: &mut Context<Self>) -> Option<Range<usize>> {
        let (point, marked) = self.ime_point();
        self.marked.as_ref().map(|_| point..point + marked)
    }

    /// Accepts the marked text as typed.
    fn unmark_text(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(text) = self.marked.take() {
            let replace = None;
            self.handle(Msg::Insert { text, replace }, cx);
        }
    }

    fn replace_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let replace = self.buffer_range(range);
        self.marked = None;
        let text = text.to_string();
        self.handle(Msg::Insert { text, replace }, cx);
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        _: Option<Range<usize>>,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(replace) = self.buffer_range(range) {
            let text = String::new();
            let replace = Some(replace);
            self.handle(Msg::Insert { text, replace }, cx);
        }
        self.marked = Some(text.to_string()).filter(|t| !t.is_empty());
        cx.notify();
    }

    fn bounds_for_range(
        &mut self,
        _: Range<usize>,
        _: Bounds<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        // shortcut: the cursor's box whatever the range, enough to place an IME's window; lay out the range if an IME needs it.
        self.painted.iter().find_map(|p| p.cursor)
    }

    fn character_index_for_point(
        &mut self,
        _: Point<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<usize> {
        None
    }
}

/// Sends a timestamp from Tokio every second, the way services message the UI, to measure that hop.
fn spawn_heartbeat(cx: &mut Context<Editor>) {
    let (tx, mut rx) = mpsc::channel(1);
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(1));
        loop {
            interval.tick().await;
            if tx.send(Instant::now()).await.is_err() {
                break;
            }
        }
    });
    cx.spawn(async move |this, cx| {
        while let Some(sent) = rx.recv().await {
            let tick = sent.elapsed();
            let alive = this.update(cx, |editor, cx| {
                if let Some(debug) = &mut editor.debug {
                    debug.tick = tick;
                }
                cx.notify();
            });
            if alive.is_err() {
                break;
            }
        }
    })
    .detach();
}

/// The text of the config file `name`; `None` when there is no file or no config dir.
fn read_config(dir: Option<&Path>, name: &str) -> Result<Option<String>, String> {
    let Some(path) = dir.map(|dir| dir.join(name)) else {
        return Ok(None);
    };
    match std::fs::read_to_string(&path) {
        Ok(text) => Ok(Some(text)),
        Err(err) if err.kind() == ErrorKind::NotFound => Ok(None),
        Err(err) => Err(format!("{}: {err}", path.display())),
    }
}

/// The key `ks` is for the core, or `None` for one that types text, which the platform's text
/// input handles so dead keys and input methods work.
fn to_key(ks: &Keystroke, option_as_meta: bool) -> Option<Key> {
    let m = ks.modifiers;
    // shortcut: Key has no Fn modifier, so Fn keys are dropped; add one when a binding needs it.
    if m.function {
        return None;
    }
    let meta = m.alt && (option_as_meta || !cfg!(target_os = "macos"));
    let code = match ks.key.as_str() {
        "enter" => KeyCode::Return,
        "tab" => KeyCode::Tab,
        "backspace" => KeyCode::Backspace,
        "escape" => KeyCode::Escape,
        "left" => KeyCode::Left,
        "right" => KeyCode::Right,
        "up" => KeyCode::Up,
        "down" => KeyCode::Down,
        // The key's own character, so C-x stays C-x whatever Option would type.
        _ if m.control || m.platform || meta || ks.key_char.is_none() => {
            let name = if ks.key == "space" { " " } else { &ks.key };
            let mut chars = name.chars();
            match (chars.next(), chars.next()) {
                (Some(c), None) => KeyCode::Char(c),
                _ => return None,
            }
        }
        _ => return None,
    };
    Some(Key {
        code,
        ctrl: m.control,
        meta,
        shift: m.shift,
        super_: m.platform,
    })
}

impl Render for Editor {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let start = Instant::now();
        let stats = self.debug.as_mut().map(|d| {
            d.frames.push_back(start);
            if d.frames.len() > 120 {
                d.frames.pop_front();
            }
            let span = start - d.frames[0];
            let fps = (d.frames.len() - 1) as f64 / span.as_secs_f64().max(f64::EPSILON);
            let ms = |d: Duration| d.as_secs_f64() * 1000.;
            format!(
                "{fps:.0} fps | render {:.2} ms | shape {:.2} ms | core {:.2} ms | tokio tick {:.2} ms",
                ms(d.render),
                ms(d.shape),
                ms(d.handle),
                ms(d.tick),
            )
        });
        let (font, cell) = self.font(window);
        let minibuffer = self.core.minibuffer();
        let settings = self.core.settings();
        let theme = &settings.theme;
        let line_height = px(settings.line_height());
        let buffer = self.core.current();
        let modified = if buffer.modified() { " *" } else { "" };
        window.set_window_title(&format!("{}{modified}", buffer.name));
        let line = || div().h(line_height).whitespace_nowrap().overflow_hidden();
        let hint = self.core.hint().map(|hint| {
            let entries: Vec<String> = (hint.entries.iter())
                .map(|(key, what)| format!("{key} {what}"))
                .collect();
            let width = entries.iter().map(|e| e.chars().count()).max().unwrap_or(0) + 2;
            let column = cell * width as f32;
            div()
                .flex()
                .flex_wrap()
                .child(line().w_full().child(format!("{}-", hint.keys)))
                .children(entries.into_iter().map(|e| line().w(column).child(e)))
        });
        let message = self.core.message().unwrap_or_default().to_string();
        let echo = match minibuffer {
            // shortcut: IME marked text is not drawn at a prompt; splice it in at the cursor when prompts take composed input.
            Some(mb) => {
                let (text, byte) = echo_line(&mb, &message);
                // Shaped whole, the cursor laid over it, so moving it never shifts the text (#11).
                let run = text_run(text.len(), Face::default(), false, &font, theme);
                let shaped = (window.text_system()).shape_line(
                    text.clone().into(),
                    px(settings.font_size),
                    &[run],
                    None,
                );
                let cursor = div()
                    .absolute()
                    .top_0()
                    .left(shaped.x_for_index(byte))
                    .w(px(2.))
                    .h(line_height)
                    .bg(rgb(theme.cursor));
                let prompt = line().relative().child(text).child(cursor);
                let rows = mb.candidates.into_iter().zip(mb.annotations).enumerate();
                let candidates = rows.map(|(i, (c, note))| {
                    let note =
                        (!note.is_empty()).then(|| div().ml(cell * 2.).opacity(0.6).child(note));
                    let row = line().flex().child(c).children(note);
                    match Some(i) == mb.selected {
                        true => row.bg(rgb(theme.region)),
                        false => row,
                    }
                });
                div().child(prompt).children(candidates)
            }
            None => div().child(line().child(message)),
        };
        let root = div()
            .track_focus(&self.focus)
            .on_key_down(cx.listener(Self::on_key_down))
            .size_full()
            .overflow_hidden()
            .flex()
            .flex_col()
            .bg(rgb(theme.background))
            .text_color(rgb(theme.foreground))
            .font_family(settings.font_family.clone().unwrap_or(FONT.into()))
            .text_size(px(settings.font_size))
            .children(stats.map(|s| line().child(s)))
            .child(
                div()
                    .flex_1()
                    .flex()
                    .overflow_hidden()
                    .on_scroll_wheel(cx.listener(Self::on_scroll))
                    .on_mouse_down(MouseButton::Left, cx.listener(Self::on_click))
                    .child(FrameElement(cx.entity())),
            )
            .children(hint)
            .child(echo);
        if let Some(debug) = &mut self.debug {
            debug.render = start.elapsed();
        }
        root
    }
}

/// A window as painted: where, its rows and mode line laid out, and the cursor.
struct Painted {
    id: WindowId,
    bounds: Bounds<Pixels>,
    selected: bool,
    rows: Vec<Row>,
    mode_line: ShapedLine,
    cursor: Option<Bounds<Pixels>>,
}

struct Row {
    /// The buffer line it starts on.
    number: usize,
    /// Its top, from the window's.
    top: Pixels,
    height: Pixels,
    decorations: Vec<Decoration>,
    text: ShapedLine,
}

/// Draws the core's frame display: every window's lines, cursor and mode line.
struct FrameElement(Entity<Editor>);

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

    /// Tells the core the size, then lays out every window's lines.
    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) -> Vec<Painted> {
        self.0.update(cx, |editor, cx| {
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
            }
            let frame = editor.core.frame();
            let start = Instant::now();
            let settings = editor.core.settings();
            let font_size = px(settings.font_size);
            let shape = |text: String, runs: &[TextRun]| {
                (window.text_system()).shape_line(text.into(), font_size, runs, None)
            };
            let painted = frame.windows.into_iter().map(|w| {
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
        let editor = self.0.read(cx);
        let focus = editor.focus.clone();
        let theme = editor.core.settings().theme.clone();
        let line_height = px(editor.core.settings().line_height());
        window.handle_input(&focus, ElementInputHandler::new(bounds, self.0.clone()), cx);
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
        self.0.update(cx, |editor, _| editor.painted = painted);
    }
}

/// `rect` from the frame display, placed at `origin`.
fn to_bounds(rect: Rect, origin: Point<Pixels>) -> Bounds<Pixels> {
    let at = point(px(rect.x), px(rect.y));
    Bounds::new(origin + at, size(px(rect.width), px(rect.height)))
}

fn text_run(len: usize, face: Face, underline: bool, font: &Font, theme: &Theme) -> TextRun {
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
fn echo_line(mb: &aiditor_core::display::Minibuffer, message: &str) -> (String, usize) {
    let spans = mb.display.lines.iter().flat_map(|l| &l.spans);
    let text: String = spans.map(|s| s.text.as_str()).collect();
    let col = mb.display.cursor.map_or(usize::MAX, |(_, col)| col);
    let byte = text.char_indices().nth(col).map_or(text.len(), |(b, _)| b);
    let message = match message.is_empty() {
        true => String::new(),
        false => format!(" [{message}]"),
    };
    (
        format!("{}{text}{message}", mb.prompt),
        mb.prompt.len() + byte,
    )
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

const USAGE: &str = "usage: aiditor [--config-dir DIR] [--debug] [FILE]";

#[derive(Debug, Default, PartialEq)]
struct Args {
    file: Option<PathBuf>,
    /// `--config-dir`, or the platform config dir.
    config_dir: Option<PathBuf>,
    /// `recovery` in the platform data dir, for auto-save files.
    recovery_dir: Option<PathBuf>,
    debug: bool,
}

fn parse_args(mut args: impl Iterator<Item = OsString>) -> Result<Args, String> {
    let mut parsed = Args::default();
    while let Some(arg) = args.next() {
        match arg.to_str() {
            Some("--debug") => parsed.debug = true,
            Some("--config-dir") => {
                parsed.config_dir = Some(PathBuf::from(
                    args.next().ok_or("--config-dir needs a DIR")?,
                ))
            }
            Some("-h" | "--help") => return Err(USAGE.into()),
            Some(flag) if flag.starts_with('-') => return Err(format!("unknown flag {flag}")),
            _ if parsed.file.is_none() => parsed.file = Some(PathBuf::from(arg)),
            _ => return Err("more than one FILE".into()),
        }
    }
    let dirs = directories::ProjectDirs::from("", "", "aiditor");
    parsed.config_dir =
        (parsed.config_dir).or_else(|| dirs.as_ref().map(|d| d.config_dir().to_owned()));
    parsed.recovery_dir = dirs.map(|d| d.data_dir().join("recovery"));
    Ok(parsed)
}

/// Sends each event `tracing` formats to the core as a `*Messages*` line.
struct LogPipe(mpsc::Sender<Msg>);

impl std::io::Write for LogPipe {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let line = String::from_utf8_lossy(buf).trim_end().to_string();
        // shortcut: a line is dropped while the channel is full; give logs their own channel if lines go missing.
        _ = self.0.try_send(Msg::Log(line));
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// The `tracing` layer that logs warnings and errors to `*Messages*`, formatted as stderr gets
/// them but without colour.
fn messages_layer<S>(tx: mpsc::Sender<Msg>) -> impl Layer<S>
where
    S: tracing::Subscriber + for<'a> LookupSpan<'a>,
{
    fmt::layer()
        .with_ansi(false)
        .with_writer(move || LogPipe(tx.clone()))
        .with_filter(LevelFilter::WARN)
}

fn main() {
    let channel = aiditor_services::channel();
    tracing_subscriber::registry()
        .with(
            EnvFilter::builder()
                .with_default_directive(LevelFilter::INFO.into())
                .from_env_lossy(),
        )
        .with(fmt::layer().with_writer(std::io::stderr))
        .with(messages_layer(channel.0.clone()))
        .init();

    let mut args = parse_args(std::env::args_os().skip(1)).unwrap_or_else(|err| {
        if err == USAGE {
            println!("{USAGE}");
            std::process::exit(0);
        }
        eprintln!("aiditor: {err}\n{USAGE}");
        std::process::exit(2);
    });
    let mut core = Core::default();
    let dir = args.config_dir.as_deref();
    core.handle(
        Instant::now(),
        Msg::SettingsLoaded(read_config(dir, "settings.toml")),
    );
    core.handle(
        Instant::now(),
        Msg::KeymapLoaded(read_config(dir, "keymap.toml")),
    );
    if let Some(arg) = args.file.take() {
        let path = std::path::absolute(&arg).unwrap_or(arg);
        let file = files::read(&path).unwrap_or_else(|err| {
            eprintln!("aiditor: {}: {err}", path.display());
            std::process::exit(1);
        });
        core.open(&path, file);
    }

    // Services run here, on background threads; GPUI owns the main thread.
    let runtime = tokio::runtime::Runtime::new().expect("failed to start the Tokio runtime");
    let _guard = runtime.enter();
    tracing::info!("starting");

    Application::new().run(|cx: &mut App| {
        let window = cx
            .open_window(WindowOptions::default(), |window, cx| {
                cx.new(|cx| Editor::new(core, args, channel, window, cx))
            })
            .expect("failed to open a window");
        window
            .update(cx, |editor, window, _| window.focus(&editor.focus))
            .expect("the window was just opened");
        cx.on_window_closed(|cx| {
            if cx.windows().is_empty() {
                cx.quit();
            }
        })
        .detach();
        cx.activate(true);
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> Result<Args, String> {
        parse_args(args.iter().map(OsString::from))
    }

    #[test]
    fn warnings_reach_the_core_without_colour_and_info_does_not() {
        let (tx, mut rx) = aiditor_services::channel();
        let subscriber = tracing_subscriber::registry().with(messages_layer(tx));
        tracing::subscriber::with_default(subscriber, || {
            tracing::info!("starting");
            tracing::warn!(n = 1, "careful");
        });
        let Ok(Msg::Log(line)) = rx.try_recv() else {
            panic!("no log line");
        };
        assert!(
            line.contains("WARN") && line.ends_with("careful n=1"),
            "{line}"
        );
        assert!(!line.contains('\x1b'), "{line}");
        assert!(rx.try_recv().is_err(), "info is not logged");
    }

    #[test]
    fn parses_flags_and_file() {
        let args = parse(&["--debug", "--config-dir", "/tmp/c", "a.txt"]).unwrap();
        assert_eq!(args.file, Some(PathBuf::from("a.txt")));
        assert_eq!(args.config_dir, Some(PathBuf::from("/tmp/c")));
        assert!(args.debug);
        assert!(!parse(&[]).unwrap().debug);
    }

    #[test]
    fn rejects_bad_arguments() {
        for bad in [&["--config-dir"][..], &["--nope"], &["a", "b"]] {
            assert!(parse(bad).is_err(), "{bad:?}");
        }
    }

    use aiditor_core::command::{ArgSpec, Command};
    use aiditor_core::key::parse_keys;
    use aiditor_core::keymap::{self, Binding, Keymap};
    use gpui::{Modifiers, ScrollDelta, TestAppContext, VisualTestContext, point};

    fn key(s: &str, option_as_meta: bool) -> Option<String> {
        to_key(&Keystroke::parse(s).unwrap(), option_as_meta).map(|k| k.to_string())
    }

    #[test]
    fn sorts_command_keys_from_typing() {
        for (stroke, want) in [
            ("ctrl-x", Some("C-x")),
            ("cmd-z", Some("s-z")),
            ("cmd-shift-z", Some("S-s-z")),
            ("ctrl-space", Some("C-SPC")),
            ("alt-backspace", Some("M-DEL")),
            ("enter", Some("RET")),
            ("alt-enter", Some("M-RET")),
            ("left", Some("<left>")),
            ("f1", None),
            ("fn-a", None),
        ] {
            assert_eq!(key(stroke, true).as_deref(), want, "{stroke}");
        }
        // A key that types, with the text the platform reports for it.
        let typed = |stroke: &str, text: &str| Keystroke {
            key_char: Some(text.into()),
            ..Keystroke::parse(stroke).unwrap()
        };
        assert_eq!(to_key(&typed("a", "a"), true), None);
        assert_eq!(to_key(&typed("shift-a", "A"), true), None);
        // Option for characters is macOS only; Linux's Alt stays Meta. macOS reports Option-e's
        // dead key as the accent and a no-break space.
        let alt_e = if cfg!(target_os = "macos") {
            None
        } else {
            Some("M-e".to_string())
        };
        let option_e = typed("alt-e", "\u{b4}\u{a0}");
        assert_eq!(to_key(&option_e, false).map(|k| k.to_string()), alt_e);
        assert_eq!(key("alt-e", true).as_deref(), Some("M-e"));
    }

    fn editor<'a>(
        cx: &'a mut TestAppContext,
        text: &str,
    ) -> (gpui::Entity<Editor>, &'a mut VisualTestContext) {
        let (editor, cx) = cx.add_window_view(|window, cx| {
            let mut core = Core::default();
            core.open(Path::new("/nonexistent/t.txt"), Some((text.into(), 0)));
            Editor::new(
                core,
                Args::default(),
                aiditor_services::channel(),
                window,
                cx,
            )
        });
        cx.update(|window, cx| window.focus(&editor.read(cx).focus));
        cx.run_until_parked();
        (editor, cx)
    }

    /// Every key sequence in `map`, in kbd form.
    fn sequences(map: &Keymap, prefix: &str, out: &mut Vec<String>) {
        for (key, binding) in map.bindings() {
            let keys = format!("{prefix}{key} ");
            match binding {
                Binding::Command(_) => out.push(keys),
                Binding::Prefix(map) => sequences(map, &keys, out),
            }
        }
    }

    /// `key` as GPUI writes keystrokes.
    fn keystroke(key: &Key) -> String {
        let mut s = String::new();
        for (held, name) in [
            (key.ctrl, "ctrl-"),
            (key.meta, "alt-"),
            (key.shift, "shift-"),
            (key.super_, "cmd-"),
        ] {
            if held {
                s += name;
            }
        }
        s + &match key.code {
            KeyCode::Char(' ') => "space".to_string(),
            KeyCode::Char(c) => c.to_string(),
            KeyCode::Return => "enter".into(),
            KeyCode::Tab => "tab".into(),
            KeyCode::Backspace => "backspace".into(),
            KeyCode::Escape => "escape".into(),
            KeyCode::Left => "left".into(),
            KeyCode::Right => "right".into(),
            KeyCode::Up => "up".into(),
            KeyCode::Down => "down".into(),
        }
    }

    #[gpui::test]
    fn every_bundled_binding_reaches_the_core(cx: &mut TestAppContext) {
        // `C-x C-s` hands the save to Tokio's blocking pool.
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let _guard = runtime.enter();
        let (editor, cx) = editor(cx, "");
        let (maps, _) = keymap::parse(keymap::BUNDLED, None, |_| Some("any")).unwrap();
        let mut script = Vec::new();
        for map in maps.values() {
            sequences(map, "", &mut script);
        }
        let want = parse_keys(&script.concat()).unwrap();
        assert!(want.len() > 50, "{want:?}");
        let strokes: Vec<String> = want.iter().map(keystroke).collect();
        editor.update(cx, |e, _| e.core.start_recording());
        cx.simulate_keystrokes(&strokes.join(" "));
        let got = editor.update(cx, |e, _| e.core.take_recording()).unwrap();
        assert_eq!(got, want);
    }

    #[gpui::test]
    fn typing_goes_through_the_text_input(cx: &mut TestAppContext) {
        let (editor, cx) = editor(cx, "");
        cx.simulate_input("aB");
        cx.simulate_keystrokes("ctrl-u 3 x ctrl-x u");
        let text = editor.read_with(cx, |e, _| e.core.text());
        assert_eq!(text, "aB", "C-u 3 x typed xxx and C-x u undid it");
        cx.simulate_keystrokes("ctrl-u 3 x");
        assert_eq!(editor.read_with(cx, |e, _| e.core.text()), "aBxxx");
    }

    #[gpui::test]
    fn closing_the_window_offers_to_save(cx: &mut TestAppContext) {
        let (editor, cx) = editor(cx, "");
        cx.simulate_input("x");
        assert!(!cx.simulate_close(), "the window stays open");
        let prompt = editor.update(cx, |e, _| e.core.minibuffer().map(|mb| mb.prompt));
        assert_eq!(prompt.as_deref(), Some("Save file /nonexistent/t.txt? "));
    }

    #[gpui::test]
    fn recovery_files_are_written_and_removed_on_quit(cx: &mut TestAppContext) {
        let dir = tempfile::tempdir().unwrap();
        let args = Args {
            recovery_dir: Some(dir.path().to_owned()),
            ..Args::default()
        };
        let channel = aiditor_services::channel();
        let (editor, cx) = cx
            .add_window_view(|window, cx| Editor::new(Core::default(), args, channel, window, cx));
        cx.update(|window, cx| window.focus(&editor.read(cx).focus));
        cx.simulate_input("x");
        // The timer runs on real time; fire it now.
        editor.update(cx, |e, cx| e.handle(Msg::AutoSave, cx));
        let count = || std::fs::read_dir(dir.path()).unwrap().count();
        for _ in 0..500 {
            if count() > 0 {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        assert_eq!(count(), 1);
        // Only `*scratch*` is modified, so this quits at once.
        editor.update(cx, |e, cx| e.handle(Msg::Quit, cx));
        assert_eq!(count(), 0);
    }

    #[gpui::test]
    fn a_prompt_reads_typed_text(cx: &mut TestAppContext) {
        let (editor, cx) = editor(cx, "");
        editor.update(cx, |e, cx| {
            e.core.register(Command {
                name: "ask",
                spec: ArgSpec {
                    prompts: &["Text: "],
                    ..ArgSpec::default()
                },
                handler: |_, args| Err(format!("got {}", args.strings[0])),
            });
            let keymap = r#"global = { "C-c p" = "ask" }"#;
            e.handle(Msg::KeymapLoaded(Ok(Some(keymap.into()))), cx);
        });
        cx.simulate_keystrokes("ctrl-c p");
        cx.simulate_input("hi");
        cx.run_until_parked();
        let prompt = editor.update(cx, |e, _| e.core.minibuffer().map(|mb| mb.prompt));
        assert_eq!(prompt.as_deref(), Some("Text: "));
        assert_eq!(editor.read_with(cx, |e, _| e.core.text()), "hi");
        cx.simulate_keystrokes("enter");
        let message = editor.read_with(cx, |e, _| e.core.message().map(str::to_string));
        assert_eq!(message.as_deref(), Some("got hi"));
        assert_eq!(editor.read_with(cx, |e, _| e.core.text()), "");
    }

    #[gpui::test]
    fn the_echo_line_is_one_string_with_point_in_it(cx: &mut TestAppContext) {
        let (editor, cx) = editor(cx, "");
        cx.simulate_keystrokes("alt-x");
        cx.simulate_input("abc");
        cx.simulate_keystrokes("ctrl-b ctrl-b alt-z");
        let line = editor.update(cx, |e, _| {
            let message = e.core.message().unwrap_or_default().to_string();
            echo_line(&e.core.minibuffer().unwrap(), &message)
        });
        let text = "M-x abc [M-z is undefined]".to_string();
        assert_eq!(line, (text, "M-x a".len()));
    }

    #[gpui::test]
    fn m_x_runs_a_command_by_name(cx: &mut TestAppContext) {
        let (editor, cx) = editor(cx, "abc");
        cx.simulate_keystrokes("alt-x");
        cx.simulate_input("forward-char");
        cx.run_until_parked();
        let prompt = editor.update(cx, |e, _| e.core.minibuffer().map(|mb| mb.prompt));
        assert_eq!(prompt.as_deref(), Some("M-x "));
        cx.simulate_keystrokes("enter");
        let last = editor.read_with(cx, |e, _| e.core.last_command());
        assert_eq!(last, Some("forward-char"));
    }

    #[gpui::test]
    fn a_completing_prompt_filters_and_picks(cx: &mut TestAppContext) {
        let (editor, cx) = editor(cx, "");
        editor.update(cx, |e, cx| {
            e.core.register(Command {
                name: "ask",
                spec: ArgSpec {
                    prompts: &["Pick: "],
                    completion: Some(|_, _| vec!["apple".into(), "banana".into()]),
                    ..ArgSpec::default()
                },
                handler: |_, args| Err(format!("got {}", args.strings[0])),
            });
            let keymap = r#"global = { "C-c p" = "ask" }"#;
            e.handle(Msg::KeymapLoaded(Ok(Some(keymap.into()))), cx);
        });
        cx.simulate_keystrokes("ctrl-c p");
        cx.simulate_input("an");
        cx.run_until_parked();
        // The matcher's threads may still be running; the core's repoll timer catches up.
        let listed = |cx: &mut VisualTestContext| {
            editor.update(cx, |e, _| e.core.minibuffer().unwrap().candidates)
        };
        for _ in 0..100 {
            if listed(cx).len() == 1 {
                break;
            }
            std::thread::sleep(Duration::from_millis(5));
            cx.executor().advance_clock(Duration::from_millis(20));
            cx.run_until_parked();
        }
        assert_eq!(listed(cx), ["banana"]);
        cx.simulate_keystrokes("enter");
        let message = editor.read_with(cx, |e, _| e.core.message().map(str::to_string));
        assert_eq!(message.as_deref(), Some("got banana"));
    }

    #[gpui::test]
    fn input_methods_compose_and_replace(cx: &mut TestAppContext) {
        let (editor, cx) = editor(cx, "😀");
        cx.simulate_keystrokes("ctrl-e");
        let state = |cx: &mut VisualTestContext| {
            editor.update_in(cx, |e, window, cx| {
                let marked = e.marked_text_range(window, cx);
                let mut adjusted = None;
                let all = e.text_for_range(0..99, &mut adjusted, window, cx);
                (e.core.text(), marked, all.unwrap())
            })
        };
        editor.update_in(cx, |e, window, cx| {
            e.replace_and_mark_text_in_range(None, "´", None, window, cx)
        });
        assert_eq!(state(cx), ("😀".into(), Some(2..3), "😀´".into()));
        editor.update_in(cx, |e, window, cx| {
            e.replace_text_in_range(Some(2..3), "é", window, cx)
        });
        assert_eq!(state(cx), ("😀é".into(), None, "😀é".into()));
        // macOS press-and-hold: the input method replaces the char just typed.
        cx.simulate_input("e");
        editor.update_in(cx, |e, window, cx| {
            e.replace_text_in_range(Some(3..4), "ê", window, cx)
        });
        assert_eq!(state(cx).0, "😀éê");
        // Unmarking accepts the marked text.
        editor.update_in(cx, |e, window, cx| {
            e.replace_and_mark_text_in_range(None, "ü", None, window, cx);
            e.unmark_text(window, cx);
        });
        assert_eq!(state(cx), ("😀éêü".into(), None, "😀éêü".into()));
    }

    #[gpui::test]
    fn cmd_c_and_cmd_v_go_through_the_clipboard(cx: &mut TestAppContext) {
        let (editor, cx) = editor(cx, "ab");
        cx.simulate_keystrokes("ctrl-space ctrl-e cmd-c");
        let copied = cx.read_from_clipboard().and_then(|item| item.text());
        assert_eq!(copied.as_deref(), Some("ab"));
        cx.write_to_clipboard(ClipboardItem::new_string("xyz".into()));
        cx.simulate_keystrokes("cmd-v");
        assert_eq!(editor.read_with(cx, |e, _| e.core.text()), "abxyz");
    }

    /// Window `i`'s bounds and its rows' line numbers and laid-out text, as last painted.
    fn painted(
        editor: &gpui::Entity<Editor>,
        cx: &mut VisualTestContext,
        i: usize,
    ) -> (Bounds<Pixels>, Vec<(usize, ShapedLine)>) {
        editor.read_with(cx, |e, _| {
            let p = &e.painted[i];
            let rows = p.rows.iter().map(|r| (r.number, r.text.clone()));
            (p.bounds, rows.collect())
        })
    }

    #[gpui::test]
    fn the_mouse_scrolls_and_moves_point(cx: &mut TestAppContext) {
        let text: String = (0..200).map(|i| format!("line {i}\n")).collect();
        let (editor, cx) = editor(cx, &text);
        let line_height = editor.read_with(cx, |e, _| px(e.core.settings().line_height()));
        let (bounds, _) = painted(&editor, cx, 0);
        let inside = bounds.origin + point(px(1.), px(1.));
        cx.simulate_event(ScrollWheelEvent {
            position: inside,
            delta: ScrollDelta::Lines(point(0., -5.)),
            ..Default::default()
        });
        let top_and_point = |cx: &mut VisualTestContext| {
            let top = painted(&editor, cx, 0).1[0].0;
            (top, editor.read_with(cx, |e, _| e.core.point()))
        };
        assert_eq!(
            top_and_point(cx),
            (5, 35),
            "point moves to the first line shown"
        );
        // Half a line twice scrolls one line.
        for _ in 0..2 {
            cx.simulate_event(ScrollWheelEvent {
                position: inside,
                delta: ScrollDelta::Pixels(point(px(0.), line_height * -0.5)),
                ..Default::default()
            });
        }
        assert_eq!(top_and_point(cx).0, 6);
        // Click over the `n` of the second line shown, `line 7`.
        let x = painted(&editor, cx, 0).1[1].1.x_for_index(2);
        let click = bounds.origin + point(x + px(1.), line_height * 1.5);
        cx.simulate_click(click, Modifiers::none());
        assert_eq!(top_and_point(cx).1, 7 * 7 + 2);
    }

    #[gpui::test]
    fn the_cursor_is_drawn_over_the_whole_line(cx: &mut TestAppContext) {
        let (editor, cx) = editor(cx, "hello world");
        let cursor = |cx: &mut VisualTestContext| {
            let (bounds, rows) = painted(&editor, cx, 0);
            let x = editor.read_with(cx, |e, _| e.painted[0].cursor.unwrap().left());
            (rows[0].1.clone(), x - bounds.left())
        };
        for col in [0, 3, 7] {
            cx.simulate_keystrokes(&format!("ctrl-a ctrl-u {col} ctrl-f"));
            let (line, x) = cursor(cx);
            assert_eq!(line.text.as_ref(), "hello world", "shaped whole at {col}");
            assert_eq!(x, line.x_for_index(col));
        }
        assert!(cursor(cx).1 > px(0.), "the test platform lays text out");
        // Marked text goes in the line, the cursor after it.
        cx.simulate_keystrokes("ctrl-a ctrl-f ctrl-f ctrl-f");
        editor.update_in(cx, |e, window, cx| {
            e.replace_and_mark_text_in_range(None, "´", None, window, cx)
        });
        cx.run_until_parked();
        let (line, x) = cursor(cx);
        assert_eq!(line.text.as_ref(), "hel´lo world");
        assert_eq!(x, line.x_for_index("hel´".len()));
    }

    #[gpui::test]
    fn clicking_another_window_selects_it(cx: &mut TestAppContext) {
        let (editor, cx) = editor(cx, "abc\ndef");
        cx.simulate_keystrokes("ctrl-x 3");
        let ((left, _), (right, rows)) = (painted(&editor, cx, 0), painted(&editor, cx, 1));
        assert_eq!(left.right(), right.left(), "side by side");
        let line_height = editor.read_with(cx, |e, _| px(e.core.settings().line_height()));
        let x = rows[1].1.x_for_index(1);
        cx.simulate_click(
            right.origin + point(x + px(1.), line_height * 1.5),
            Modifiers::none(),
        );
        let (point, selected) = editor.update(cx, |e, _| {
            let frame = e.core.frame();
            (
                e.core.point(),
                frame.windows.iter().map(|w| w.selected).collect::<Vec<_>>(),
            )
        });
        assert_eq!((point, selected), (5, vec![false, true]));
    }
}
