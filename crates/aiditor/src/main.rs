use std::io::ErrorKind;
use std::path::PathBuf;
use std::time::Instant;

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
    /// The first line drawn; moves only to keep point's line on screen.
    top: usize,
    #[expect(
        dead_code,
        reason = "keeps the channel open; effects will clone it into services"
    )]
    tx: mpsc::Sender<Msg>,
    /// Wakes the core at its next deadline; replacing it cancels the old one.
    timer: Option<Task<()>>,
}

impl Editor {
    fn new(core: Core, cx: &mut Context<Self>) -> Self {
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
        Editor {
            core,
            focus: cx.focus_handle(),
            top: 0,
            tx,
            timer: None,
        }
    }

    /// Every message reaches the core here, so the timer is always re-armed.
    fn handle(&mut self, msg: Msg, cx: &mut Context<Self>) {
        for effect in self.core.handle(Instant::now(), msg) {
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
                Effect::LoadSettings => self.handle(Msg::SettingsLoaded(read_settings()), cx),
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

/// The user's `settings.toml` in the platform config dir; `None` when it or the dir doesn't exist.
fn read_settings() -> Result<Option<String>, String> {
    let Some(dirs) = directories::ProjectDirs::from("", "", "aiditor") else {
        return Ok(None);
    };
    let path = dirs.config_dir().join("settings.toml");
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
        let settings = self.core.settings();
        let theme = &settings.theme;
        let line_height = px(settings.font_size * 1.4);
        let message = self.core.message().map(str::to_string);
        let buffer = self.core.current();
        let modified = if buffer.modified() { " *" } else { "" };
        window.set_window_title(&format!("{}{modified}", buffer.name));
        let height = window.viewport_size().height
            - if message.is_some() {
                line_height
            } else {
                px(0.)
            };
        let rows = ((height / line_height) as usize).max(1);
        let (line, col) = buffer.line_col(self.core.point()).unwrap_or_default();
        self.top = self.top.clamp(line.saturating_sub(rows - 1), line);
        let text = buffer.text();
        // shortcut: whole lines as div text, no tabs or horizontal scroll; the display-model renderer replaces it.
        let lines = (self.top..text.len_lines().min(self.top + rows)).map(|i| {
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
        div()
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
            .child(div().flex_1().overflow_hidden().children(lines))
            .children(message.map(|m| div().h(line_height).whitespace_nowrap().child(m)))
    }
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

    let mut core = Core::default();
    core.handle(Instant::now(), Msg::SettingsLoaded(read_settings()));
    if let Some(arg) = std::env::args_os().nth(1) {
        let path = std::path::absolute(&arg).unwrap_or_else(|_| PathBuf::from(arg));
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
                cx.new(|cx| Editor::new(core, cx))
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
