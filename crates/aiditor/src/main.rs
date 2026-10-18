use std::collections::VecDeque;
use std::ffi::OsString;
use std::io::ErrorKind;
use std::ops::Range;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use aiditor_core::buffer::Buffer;
use aiditor_core::key::{Key, KeyCode};
use aiditor_core::{Core, Effect, Msg};
use gpui::{
    App, Application, Bounds, Context, ElementInputHandler, EntityInputHandler, FocusHandle,
    KeyDownEvent, Keystroke, MouseButton, MouseDownEvent, Pixels, Point, ScrollWheelEvent,
    ShapedLine, Task, TextRun, UTF16Selection, Window, WindowOptions, canvas, div, font,
    prelude::*, px, rgb,
};
use tokio::sync::mpsc;
use tracing_subscriber::{EnvFilter, filter::LevelFilter};

const FONT: &str = if cfg!(target_os = "macos") {
    "Menlo"
} else {
    "Noto Sans Mono"
};

struct Editor {
    core: Core,
    focus: FocusHandle,
    #[expect(
        dead_code,
        reason = "keeps the channel open; effects will clone it into services"
    )]
    tx: mpsc::Sender<Msg>,
    /// Wakes the core at its next deadline; replacing it cancels the old one.
    timer: Option<Task<()>>,
    /// Where the user's settings and keymap files are, if known.
    config_dir: Option<PathBuf>,
    /// Set by `--debug`: the stats line drawn at the top.
    debug: Option<Debug>,
    /// shortcut: the input method's marked text lives here, drawn at point; #11 moves it into the window's display state.
    marked: Option<String>,
    /// Where the buffer text was last painted, for clicks and the input method.
    text_bounds: Bounds<Pixels>,
    /// The first buffer line last drawn.
    top: usize,
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
    /// How long the last heartbeat took from a Tokio task to the UI thread.
    tick: Duration,
}

impl Editor {
    fn new(core: Core, args: Args, cx: &mut Context<Self>) -> Self {
        let (tx, mut rx) = aiditor_services::channel();
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
            debug: args.debug.then(Debug::default),
            marked: None,
            text_bounds: Bounds::default(),
            top: 0,
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
                // shortcut: a synchronous, non-atomic write on the UI thread; the file service (#46) replaces it.
                Effect::Save { buffer, path, text } => {
                    match std::fs::write(&path, text.to_string()) {
                        Ok(()) => {
                            tracing::info!(path = %path.display(), "saved");
                            self.handle(Msg::Saved(buffer), cx);
                        }
                        Err(err) => tracing::error!(path = %path.display(), %err, "save failed"),
                    }
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
            self.handle(Msg::Key(key), cx);
        }
    }

    fn on_scroll(&mut self, ev: &ScrollWheelEvent, _: &mut Window, cx: &mut Context<Self>) {
        let line_height = px(self.core.settings().line_height());
        // GPUI's delta is negative when the content should move up, that is down the buffer.
        self.scroll_px -= ev.delta.pixel_delta(line_height).y;
        let lines = (self.scroll_px / line_height) as i64;
        if lines != 0 {
            self.scroll_px -= line_height * lines as f32;
            self.handle(Msg::Scroll(lines), cx);
        }
    }

    fn on_click(&mut self, ev: &MouseDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(p) = self.char_at(ev.position, window) {
            self.handle(Msg::Click(p), cx);
        }
    }

    /// The char offset nearest `pos`, or the end of the line or buffer past them.
    fn char_at(&self, pos: Point<Pixels>, window: &Window) -> Option<usize> {
        let bounds = self.text_bounds;
        if !bounds.contains(&pos) {
            return None;
        }
        let line_height = px(self.core.settings().line_height());
        let line = self.top + ((pos.y - bounds.top()) / line_height) as usize;
        let buffer = self.core.current();
        if line >= buffer.len_lines() {
            return Some(buffer.len_chars());
        }
        let text = line_text(buffer, line);
        let byte = self
            .shape(&text, window)
            .closest_index_for_x(pos.x - bounds.left());
        buffer.char_idx(line, text[..byte].chars().count()).ok()
    }

    /// `text` laid out in the buffer font, as the frame draws it.
    fn shape(&self, text: &str, window: &Window) -> ShapedLine {
        let settings = self.core.settings();
        let run = TextRun {
            len: text.len(),
            font: font(settings.font_family.clone().unwrap_or(FONT.into())),
            color: rgb(settings.theme.foreground).into(),
            background_color: None,
            underline: None,
            strikethrough: None,
        };
        window.text_system().shape_line(
            text.to_string().into(),
            px(settings.font_size),
            &[run],
            None,
        )
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

/// The buffer's line `line` without its newline.
fn line_text(buffer: &Buffer, line: usize) -> String {
    buffer
        .text()
        .line(line)
        .to_string()
        .trim_end_matches('\n')
        .to_string()
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
        window: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        // shortcut: the box at point whatever the range, enough to place an IME's window; use the range once #11 lays out lines.
        let buffer = self.core.current();
        let (line, col) = buffer.line_col(self.core.point()).ok()?;
        let text = line_text(buffer, line);
        let byte = text.char_indices().nth(col).map_or(text.len(), |(b, _)| b);
        let line_height = px(self.core.settings().line_height());
        let origin = self.text_bounds.origin
            + Point::new(
                self.shape(&text, window).x_for_index(byte),
                line_height * line.saturating_sub(self.top) as f32,
            );
        Some(Bounds::new(origin, gpui::size(px(1.), line_height)))
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
            format!(
                "{fps:.0} fps | render {:.2} ms | core {:.2} ms | tokio tick {:.2} ms",
                d.render.as_secs_f64() * 1000.,
                d.handle.as_secs_f64() * 1000.,
                d.tick.as_secs_f64() * 1000.,
            )
        });
        let line_height = px(self.core.settings().line_height());
        let message = self.core.message().map(str::to_string);
        // shortcut: the hint on one clipped line; the display-model renderer lays it out in columns.
        let hint = self.core.hint().map(|hint| {
            let entries = hint
                .entries
                .iter()
                .map(|(key, what)| format!("{key} {what}"));
            format!(
                "{}-  {}",
                hint.keys,
                entries.collect::<Vec<_>>().join("   ")
            )
        });
        let bars = message.is_some() as usize + hint.is_some() as usize + stats.is_some() as usize;
        let height = window.viewport_size().height - line_height * bars as f32;
        let rows = ((height / line_height) as usize).max(1);
        let top = self.core.scroll(rows);
        self.top = top;
        let marked = self.marked.clone();
        let settings = self.core.settings();
        let theme = &settings.theme;
        let buffer = self.core.current();
        let modified = if buffer.modified() { " *" } else { "" };
        window.set_window_title(&format!("{}{modified}", buffer.name));
        let (line, col) = buffer.line_col(self.core.point()).unwrap_or_default();
        let text = buffer.text();
        // shortcut: whole lines as div text, no tabs or horizontal scroll; the display-model renderer replaces it.
        let lines = (top..text.len_lines().min(top + rows)).map(|i| {
            let s = text.line(i).to_string();
            let s = s.trim_end_matches('\n');
            let row = div().h(line_height).flex().whitespace_nowrap();
            if i != line {
                return row.child(s.to_string());
            }
            let split = s.char_indices().nth(col).map_or(s.len(), |(b, _)| b);
            row.child(s[..split].to_string())
                .children(marked.clone().map(|m| div().underline().child(m)))
                .child(div().w(px(2.)).h_full().bg(rgb(theme.cursor)))
                .child(s[split..].to_string())
        });
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
            .children(stats.map(|s| div().h(line_height).whitespace_nowrap().child(s)))
            .child(
                div()
                    .flex_1()
                    .relative()
                    .overflow_hidden()
                    .on_scroll_wheel(cx.listener(Self::on_scroll))
                    .on_mouse_down(MouseButton::Left, cx.listener(Self::on_click))
                    .children(lines)
                    .child(
                        canvas(|_, _, _| {}, input_handler(cx))
                            .absolute()
                            .top_0()
                            .left_0()
                            .size_full(),
                    ),
            )
            .children(hint.map(|h| div().h(line_height).whitespace_nowrap().child(h)))
            .children(message.map(|m| div().h(line_height).whitespace_nowrap().child(m)));
        if let Some(debug) = &mut self.debug {
            debug.render = start.elapsed();
        }
        root
    }
}

/// Paints nothing: registers the editor as the window's text input and keeps where the text is.
fn input_handler(
    cx: &mut Context<Editor>,
) -> impl FnOnce(Bounds<Pixels>, (), &mut Window, &mut App) + 'static {
    let editor = cx.entity();
    move |bounds, _, window, cx| {
        let focus = editor.read(cx).focus.clone();
        window.handle_input(&focus, ElementInputHandler::new(bounds, editor.clone()), cx);
        editor.update(cx, |editor, _| editor.text_bounds = bounds);
    }
}

const USAGE: &str = "usage: aiditor [--config-dir DIR] [--debug] [FILE]";

#[derive(Debug, Default, PartialEq)]
struct Args {
    file: Option<PathBuf>,
    /// `--config-dir`, or the platform config dir.
    config_dir: Option<PathBuf>,
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
    parsed.config_dir = parsed.config_dir.or_else(|| {
        directories::ProjectDirs::from("", "", "aiditor").map(|d| d.config_dir().to_owned())
    });
    Ok(parsed)
}

fn main() {
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(
            EnvFilter::builder()
                .with_default_directive(LevelFilter::INFO.into())
                .from_env_lossy(),
        )
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
        let text = match std::fs::read_to_string(&path) {
            Ok(text) => text,
            Err(err) if err.kind() == ErrorKind::NotFound => String::new(),
            Err(err) => {
                eprintln!("aiditor: {}: {err}", path.display());
                std::process::exit(1);
            }
        };
        core.open(&path, &text);
    }

    // Services run here, on background threads; GPUI owns the main thread.
    let runtime = tokio::runtime::Runtime::new().expect("failed to start the Tokio runtime");
    let _guard = runtime.enter();
    tracing::info!("starting");

    Application::new().run(|cx: &mut App| {
        let window = cx
            .open_window(WindowOptions::default(), |_, cx| {
                cx.new(|cx| Editor::new(core, args, cx))
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
        let (editor, cx) = cx.add_window_view(|_, cx| {
            let mut core = Core::default();
            core.open(Path::new("/nonexistent/t.txt"), text);
            Editor::new(core, Args::default(), cx)
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
    fn the_mouse_scrolls_and_moves_point(cx: &mut TestAppContext) {
        let text: String = (0..200).map(|i| format!("line {i}\n")).collect();
        let (editor, cx) = editor(cx, &text);
        let (bounds, line_height) = editor.read_with(cx, |e, _| {
            (e.text_bounds, px(e.core.settings().line_height()))
        });
        let inside = bounds.origin + point(px(1.), px(1.));
        cx.simulate_event(ScrollWheelEvent {
            position: inside,
            delta: ScrollDelta::Lines(point(0., -5.)),
            ..Default::default()
        });
        let point_at =
            |cx: &mut VisualTestContext| editor.read_with(cx, |e, _| (e.top, e.core.point()));
        assert_eq!(point_at(cx), (5, 35), "point moves to the first line shown");
        // Half a line twice scrolls one line.
        for _ in 0..2 {
            cx.simulate_event(ScrollWheelEvent {
                position: inside,
                delta: ScrollDelta::Pixels(point(px(0.), line_height * -0.5)),
                ..Default::default()
            });
        }
        assert_eq!(point_at(cx).0, 6);
        // Click over the `n` of the second line shown, `line 7`.
        let x = editor.update_in(cx, |e, window, _| e.shape("line 7", window).x_for_index(2));
        let click = bounds.origin + point(x + px(1.), line_height * 1.5);
        cx.simulate_click(click, Modifiers::none());
        assert_eq!(point_at(cx).1, 7 * 7 + 2);
    }
}
