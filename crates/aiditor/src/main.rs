use std::collections::VecDeque;
use std::ffi::OsString;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use aiditor_core::key::{Key, KeyCode};
use aiditor_core::{Core, Effect, Msg};
use gpui::{
    App, Application, Context, FocusHandle, KeyDownEvent, Keystroke, Task, Window, WindowOptions,
    div, prelude::*, px, rgb,
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

    fn on_key_down(&mut self, ev: &KeyDownEvent, _: &mut Window, cx: &mut Context<Self>) {
        let keys = to_keys(&ev.keystroke);
        if !keys.is_empty() {
            cx.stop_propagation();
        }
        for key in keys {
            self.handle(Msg::Key(key), cx);
        }
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

/// shortcut: typing is consumed as keys, so dead keys and IME don't work; #12 routes it through the input handler.
fn to_keys(ks: &Keystroke) -> Vec<Key> {
    let m = ks.modifiers;
    if m.platform || m.function {
        return Vec::new();
    }
    let key = |code| Key {
        code,
        ctrl: m.control,
        meta: m.alt,
        shift: m.shift,
    };
    let named = match ks.key.as_str() {
        "enter" => KeyCode::Return,
        "tab" => KeyCode::Tab,
        "backspace" => KeyCode::Backspace,
        "escape" => KeyCode::Escape,
        "left" => KeyCode::Left,
        "right" => KeyCode::Right,
        "up" => KeyCode::Up,
        "down" => KeyCode::Down,
        // With Ctrl or Alt held, the key's own character, so C-x stays C-x whatever Option would type.
        _ if m.control || m.alt => {
            let mut chars = ks.key.chars();
            return match (chars.next(), chars.next()) {
                (Some(c), None) => vec![key(KeyCode::Char(c))],
                _ => Vec::new(),
            };
        }
        _ => {
            return ks
                .key_char
                .iter()
                .flat_map(|s| s.chars())
                .map(|c| Key::plain(KeyCode::Char(c)))
                .collect();
        }
    };
    vec![key(named)]
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
            .child(div().flex_1().overflow_hidden().children(lines))
            .children(hint.map(|h| div().h(line_height).whitespace_nowrap().child(h)))
            .children(message.map(|m| div().h(line_height).whitespace_nowrap().child(m)));
        if let Some(debug) = &mut self.debug {
            debug.render = start.elapsed();
        }
        root
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

    #[test]
    fn undo_and_redo_keys_reach_the_core_with_ctrl() {
        let key = |s| to_keys(&Keystroke::parse(s).unwrap());
        let ctrl = |c, shift| Key {
            ctrl: true,
            shift,
            ..Key::plain(KeyCode::Char(c))
        };
        assert_eq!(key("ctrl-/"), [ctrl('/', false)]);
        // As GPUI on Linux reports Ctrl+Shift+/; the core ignores the shift.
        assert_eq!(key("ctrl-shift-?"), [ctrl('?', true)]);
    }

    #[test]
    fn prefix_argument_keys_reach_the_core() {
        let key = |s| to_keys(&Keystroke::parse(s).unwrap());
        let keys = [key("ctrl-u"), key("ctrl--"), key("alt-3")].concat();
        assert_eq!(keys, aiditor_core::key::parse_keys("C-u C-- M-3").unwrap());
    }

    #[test]
    fn motion_keys_reach_the_core() {
        let key = |s| to_keys(&Keystroke::parse(s).unwrap());
        let keys = [key("ctrl-f"), key("alt-<"), key("alt-shift->"), key("left")].concat();
        let script = "C-f M-< M-S-> <left>";
        assert_eq!(keys, aiditor_core::key::parse_keys(script).unwrap());
    }

    #[test]
    fn editing_keys_reach_the_core() {
        let key = |s| to_keys(&Keystroke::parse(s).unwrap());
        let keys = [
            "enter",
            "ctrl-j",
            "ctrl-o",
            "ctrl-d",
            "backspace",
            "alt-d",
            "alt-backspace",
        ]
        .map(key)
        .concat();
        let script = "RET C-j C-o C-d DEL M-d M-DEL";
        assert_eq!(keys, aiditor_core::key::parse_keys(script).unwrap());
    }
}
