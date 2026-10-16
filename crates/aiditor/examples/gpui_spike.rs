//! Throwaway spike for issue #7: drawing, raw keys, Option/dead keys, IME, Tokio -> GPUI.
//! Run with `cargo run -p aiditor --example gpui_spike`. Events are logged in the window and on stderr.
//! F1 toggles Option-as-Meta, F2 toggles the frame-rate stress view, Cmd-Q quits. `SPIKE_STRESS=1` starts in the stress view.

use std::collections::VecDeque;
use std::ops::Range;
use std::time::{Duration, Instant};

use gpui::{
    App, Application, Bounds, Context, Element, ElementId, ElementInputHandler, Entity,
    EntityInputHandler, FocusHandle, Font, GlobalElementId, Hsla, KeyDownEvent, LayoutId, Pixels,
    Point, ShapedLine, Style, TextRun, UTF16Selection, UnderlineStyle, Window, WindowOptions, div,
    fill, font, hsla, point, prelude::*, px, relative, rgb, size,
};

const FONT_SIZE: f32 = 14.;
const LINE_HEIGHT: f32 = 20.;
const FONT: &str = if cfg!(target_os = "macos") {
    "Menlo"
} else {
    "Noto Sans Mono"
};

struct Spike {
    focus: FocusHandle,
    text: String,
    /// Byte offset into `text`.
    cursor: usize,
    marked: Option<Range<usize>>,
    option_as_meta: bool,
    /// Generated lines scrolled every frame, or None for the editable buffer.
    stress: Option<Vec<String>>,
    scroll: usize,
    log: VecDeque<String>,
    frames: VecDeque<Instant>,
    shape_time: Duration,
    ticks: u64,
    tick_latency: Duration,
    tick_thread: String,
    /// Each painted line's start offset and layout, for the IME's bounds queries.
    layout: Vec<(usize, ShapedLine)>,
    origin: Point<Pixels>,
}

impl Spike {
    fn log(&mut self, line: String) {
        eprintln!("{line}");
        self.log.push_back(line);
        if self.log.len() > 18 {
            self.log.pop_front();
        }
    }

    fn insert(&mut self, range: Range<usize>, s: &str) {
        self.text.replace_range(range.clone(), s);
        self.cursor = range.start + s.len();
    }

    fn on_key_down(&mut self, ev: &KeyDownEvent, _: &mut Window, cx: &mut Context<Self>) {
        let ks = &ev.keystroke;
        let m = ks.modifiers;
        let named = matches!(
            ks.key.as_str(),
            "enter" | "tab" | "backspace" | "delete" | "escape"
        );
        // Plain printable keys must propagate so the platform input handler (and dead keys) sees them.
        let consume = m.control
            || m.platform
            || m.function
            || (m.alt && self.option_as_meta)
            || ks.key_char.is_none()
            || named;
        self.log(format!(
            "key-down key={:?} key_char={:?} ctrl={} alt={} shift={} cmd={} fn={} held={} -> {}",
            ks.key,
            ks.key_char,
            m.control,
            m.alt,
            m.shift,
            m.platform,
            m.function,
            ev.is_held,
            if consume { "Key" } else { "propagate" }
        ));
        if !consume {
            return;
        }
        cx.stop_propagation();
        match ks.key.as_str() {
            "f1" => self.option_as_meta = !self.option_as_meta,
            "f2" => {
                self.stress = match self.stress {
                    Some(_) => None,
                    None => Some(stress_lines()),
                };
            }
            "q" if m.platform => cx.quit(),
            "enter" => self.insert(self.cursor..self.cursor, "\n"),
            "backspace" => {
                if let Some(c) = self.text[..self.cursor].chars().next_back() {
                    self.insert(self.cursor - c.len_utf8()..self.cursor, "");
                }
            }
            "left" => {
                if let Some(c) = self.text[..self.cursor].chars().next_back() {
                    self.cursor -= c.len_utf8();
                }
            }
            "right" => {
                if let Some(c) = self.text[self.cursor..].chars().next() {
                    self.cursor += c.len_utf8();
                }
            }
            _ => {}
        }
        cx.notify();
    }

    fn utf16_to_byte(&self, utf16: usize) -> usize {
        let mut n = 0;
        for (i, c) in self.text.char_indices() {
            if n >= utf16 {
                return i;
            }
            n += c.len_utf16();
        }
        self.text.len()
    }

    fn byte_to_utf16(&self, byte: usize) -> usize {
        self.text[..byte].encode_utf16().count()
    }

    fn range_from_utf16(&self, r: &Range<usize>) -> Range<usize> {
        self.utf16_to_byte(r.start)..self.utf16_to_byte(r.end)
    }

    fn range_to_utf16(&self, r: &Range<usize>) -> Range<usize> {
        self.byte_to_utf16(r.start)..self.byte_to_utf16(r.end)
    }

    /// The range the IME means: the one it names, else the marked text, else the cursor.
    fn target(&self, range_utf16: &Option<Range<usize>>) -> Range<usize> {
        range_utf16
            .as_ref()
            .map(|r| self.range_from_utf16(r))
            .or(self.marked.clone())
            .unwrap_or(self.cursor..self.cursor)
    }
}

impl EntityInputHandler for Spike {
    fn text_for_range(
        &mut self,
        range_utf16: Range<usize>,
        actual: &mut Option<Range<usize>>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<String> {
        let range = self.range_from_utf16(&range_utf16);
        actual.replace(self.range_to_utf16(&range));
        self.log(format!("ime text_for_range {range_utf16:?}"));
        Some(self.text[range].to_string())
    }

    fn selected_text_range(
        &mut self,
        _: bool,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        let c = self.byte_to_utf16(self.cursor);
        Some(UTF16Selection {
            range: c..c,
            reversed: false,
        })
    }

    fn marked_text_range(&self, _: &mut Window, _: &mut Context<Self>) -> Option<Range<usize>> {
        self.marked.as_ref().map(|r| self.range_to_utf16(r))
    }

    fn unmark_text(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        self.log("ime unmark_text".into());
        self.marked = None;
        cx.notify();
    }

    fn replace_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        text: &str,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.log(format!(
            "ime replace_text_in_range {range_utf16:?} {text:?}"
        ));
        let range = self.target(&range_utf16);
        self.insert(range, text);
        self.marked = None;
        cx.notify();
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        text: &str,
        selected_utf16: Option<Range<usize>>,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.log(format!(
            "ime replace_and_mark {range_utf16:?} {text:?} selected={selected_utf16:?}"
        ));
        let range = self.target(&range_utf16);
        self.insert(range.clone(), text);
        self.marked = (!text.is_empty()).then(|| range.start..range.start + text.len());
        cx.notify();
    }

    fn bounds_for_range(
        &mut self,
        range_utf16: Range<usize>,
        _: Bounds<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        let start = self.utf16_to_byte(range_utf16.start);
        let i = self.layout.iter().rposition(|(s, _)| *s <= start)?;
        let (line_start, line) = &self.layout[i];
        let x = self.origin.x + line.x_for_index(start - line_start);
        let y = self.origin.y + px(LINE_HEIGHT) * i as f32;
        Some(Bounds::new(point(x, y), size(px(2.), px(LINE_HEIGHT))))
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

impl Render for Spike {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.frames.push_back(Instant::now());
        if self.frames.len() > 120 {
            self.frames.pop_front();
        }
        let fps = match (self.frames.front(), self.frames.back()) {
            (Some(a), Some(b)) if b > a => (self.frames.len() - 1) as f64 / (*b - *a).as_secs_f64(),
            _ => 0.,
        };
        if self.stress.is_some() {
            self.scroll += 1;
            window.request_animation_frame();
        }
        let status = format!(
            "F1 option-as-meta={} | F2 stress={} | {:.0} fps, shaping {:.2} ms | tokio ticks={} latency={:?} from {}",
            self.option_as_meta,
            self.stress.is_some(),
            fps,
            self.shape_time.as_secs_f64() * 1000.,
            self.ticks,
            self.tick_latency,
            self.tick_thread,
        );
        div()
            .track_focus(&self.focus)
            .on_key_down(cx.listener(Self::on_key_down))
            .size_full()
            .flex()
            .flex_col()
            .bg(rgb(0x1e1e1e))
            .text_color(rgb(0xd4d4d4))
            .font_family(FONT)
            .text_size(px(12.))
            .child(div().bg(rgb(0x333333)).px_2().child(status))
            .child(BufferElement { view: cx.entity() })
            .child(
                div()
                    .bg(rgb(0x252525))
                    .px_2()
                    .flex()
                    .flex_col()
                    .children(self.log.iter().cloned()),
            )
    }
}

struct BufferElement {
    view: Entity<Spike>,
}

impl IntoElement for BufferElement {
    type Element = Self;

    fn into_element(self) -> Self {
        self
    }
}

impl Element for BufferElement {
    type RequestLayoutState = ();
    type PrepaintState = (Vec<(usize, ShapedLine)>, Option<Bounds<Pixels>>);

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&gpui::InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, ()) {
        let mut style = Style::default();
        style.size.width = relative(1.).into();
        style.flex_grow = 1.;
        (window.request_layout(style, [], cx), ())
    }

    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&gpui::InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        let started = Instant::now();
        let view = self.view.read(cx);
        let font = font(FONT);
        let rows = (bounds.size.height / px(LINE_HEIGHT)) as usize;
        let mut lines = Vec::new();
        let mut cursor = None;
        match &view.stress {
            Some(stress) => {
                for i in 0..rows {
                    let text = &stress[(view.scroll + i) % stress.len()];
                    lines.push((0, shape(window, text, 0, None, &font)));
                }
            }
            None => {
                let mut start = 0;
                for text in view.text.split('\n').take(rows) {
                    let line = shape(window, text, start, view.marked.as_ref(), &font);
                    if (start..=start + text.len()).contains(&view.cursor) {
                        let x = line.x_for_index(view.cursor - start);
                        let y = px(LINE_HEIGHT) * lines.len() as f32;
                        cursor = Some(Bounds::new(
                            bounds.origin + point(x, y),
                            size(px(2.), px(LINE_HEIGHT)),
                        ));
                    }
                    lines.push((start, line));
                    start += text.len() + 1;
                }
            }
        }
        let elapsed = started.elapsed();
        self.view.update(cx, |v, _| v.shape_time = elapsed);
        (lines, cursor)
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&gpui::InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut (),
        (lines, cursor): &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        let focus = self.view.read(cx).focus.clone();
        window.handle_input(
            &focus,
            ElementInputHandler::new(bounds, self.view.clone()),
            cx,
        );
        for (i, (_, line)) in lines.iter().enumerate() {
            let origin = bounds.origin + point(px(0.), px(LINE_HEIGHT) * i as f32);
            line.paint_background(origin, px(LINE_HEIGHT), window, cx)
                .unwrap();
            line.paint(origin, px(LINE_HEIGHT), window, cx).unwrap();
        }
        if let Some(cursor) = cursor.take() {
            window.paint_quad(fill(cursor, rgb(0xffcc00)));
        }
        let lines = std::mem::take(lines);
        self.view.update(cx, |v, _| {
            v.layout = lines;
            v.origin = bounds.origin;
        });
    }
}

/// Shapes one line with a styled span per token, underlining the IME's marked text.
fn shape(
    window: &mut Window,
    text: &str,
    start: usize,
    marked: Option<&Range<usize>>,
    font: &Font,
) -> ShapedLine {
    let mut cuts: Vec<usize> = text
        .char_indices()
        .filter(|(i, c)| *i > 0 && (c.is_whitespace() || c.is_ascii_punctuation()))
        .map(|(i, _)| i)
        .collect();
    if let Some(m) = marked {
        for b in [m.start, m.end] {
            if b > start && b < start + text.len() {
                cuts.push(b - start);
            }
        }
    }
    cuts.push(text.len());
    cuts.sort();
    cuts.dedup();
    let mut runs = Vec::new();
    let mut prev = 0;
    for cut in cuts {
        if cut == prev {
            continue;
        }
        let seg = &text[prev..cut];
        let underlined = marked.is_some_and(|m| m.start <= start + prev && start + cut <= m.end);
        runs.push(TextRun {
            len: cut - prev,
            font: font.clone(),
            color: token_color(seg),
            background_color: seg
                .trim_start()
                .starts_with(char::is_numeric)
                .then(|| hsla(0.1, 0.5, 0.2, 1.)),
            underline: underlined.then(|| UnderlineStyle {
                color: Some(hsla(0.15, 1., 0.6, 1.)),
                thickness: px(2.),
                wavy: false,
            }),
            strikethrough: None,
        });
        prev = cut;
    }
    window
        .text_system()
        .shape_line(text.to_string().into(), px(FONT_SIZE), &runs, None)
}

fn token_color(seg: &str) -> Hsla {
    let word = seg.trim_start();
    match word.chars().next() {
        Some(c) if c.is_ascii_punctuation() => hsla(0., 0., 0.6, 1.),
        Some(c) if c.is_numeric() => hsla(0.08, 0.8, 0.65, 1.),
        Some(_) => hsla((word.len() % 6) as f32 / 6., 0.6, 0.7, 1.),
        None => hsla(0., 0., 0.8, 1.),
    }
}

fn stress_lines() -> Vec<String> {
    (0..500)
        .map(|i| {
            format!(
                "{i:4} fn handle_{i}(core: &mut Core, msg: Msg) -> Effects {{ let x = {} * {i}; core.apply(x, \"span\"); }}",
                i * 7
            )
        })
        .collect()
}

fn main() {
    let runtime = tokio::runtime::Runtime::new().expect("failed to start the Tokio runtime");
    let (tx, mut rx) = tokio::sync::mpsc::channel::<(Instant, String)>(8);
    runtime.spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_millis(250));
        loop {
            interval.tick().await;
            let thread = std::thread::current().name().unwrap_or("?").to_string();
            if tx.send((Instant::now(), thread)).await.is_err() {
                break;
            }
        }
    });

    Application::new().run(move |cx: &mut App| {
        cx.intercept_keystrokes(|ev, _, _| {
            eprintln!(
                "intercept {} (before keymap and key-down)",
                ev.keystroke.unparse()
            )
        })
        .detach();
        let window = cx
            .open_window(WindowOptions::default(), |_, cx| {
                cx.new(|cx| {
                    cx.spawn(async move |this, cx| {
                        while let Some((sent, thread)) = rx.recv().await {
                            let latency = sent.elapsed();
                            let alive = this.update(cx, |s: &mut Spike, cx| {
                                s.ticks += 1;
                                s.tick_latency = latency;
                                s.tick_thread = thread;
                                cx.notify();
                            });
                            if alive.is_err() {
                                break;
                            }
                        }
                    })
                    .detach();
                    Spike {
                        focus: cx.focus_handle(),
                        text: "Type here. Try Option-e e, Option-s, C-x, M-f, Cmd-C, arrows.\n"
                            .into(),
                        cursor: 0,
                        marked: None,
                        option_as_meta: false,
                        stress: std::env::var_os("SPIKE_STRESS").map(|_| stress_lines()),
                        scroll: 0,
                        log: VecDeque::new(),
                        frames: VecDeque::new(),
                        shape_time: Duration::ZERO,
                        ticks: 0,
                        tick_latency: Duration::ZERO,
                        tick_thread: String::new(),
                        layout: Vec::new(),
                        origin: Point::default(),
                    }
                })
            })
            .expect("failed to open a window");
        window
            .update(cx, |view, window, cx| {
                window.focus(&view.focus);
                cx.activate(true);
            })
            .unwrap();
        cx.on_window_closed(|cx| {
            if cx.windows().is_empty() {
                cx.quit();
            }
        })
        .detach();
    });
}
