pub(crate) mod benchmark;

use std::collections::VecDeque;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use aiditor_core::buffer::Face;
use aiditor_core::{Core, Effect, Msg, RecoveryOp};
use aiditor_services::{
    agents, configuration, environment, files, grep, recovery, terminal, watcher,
};
use gpui::{
    ClipboardItem, Context, FocusHandle, Font, KeyDownEvent, Pixels, Task, Window, div, font,
    prelude::*, px, rgb,
};
use tokio::sync::mpsc;

use crate::element::{FrameElement, Painted, block, echo_line, text_run};
use crate::input::to_key;
use crate::{Args, FONT};

pub(crate) struct Editor {
    pub(crate) core: Core,
    pub(crate) benchmark: Option<benchmark::Run>,
    pub(crate) focus: FocusHandle,
    /// Services send their results to the core on this.
    tx: mpsc::Sender<Msg>,
    /// Wakes the core at its next deadline; replacing it cancels the old one.
    timer: Option<Task<()>>,
    /// Where the user's settings and keymap files are, if known.
    config_dir: Option<PathBuf>,
    grep: grep::Grep,
    terminals: terminal::Terminals,
    agents: agents::Agents,
    watcher: watcher::Watcher,
    /// The recovery service, when there is a data dir.
    recovery: Option<(
        std::sync::mpsc::Sender<RecoveryOp>,
        std::thread::JoinHandle<()>,
    )>,
    /// Set by `--debug`: the stats line drawn at the top.
    pub(crate) debug: Option<Stats>,
    /// The input method's marked text, drawn at point but not in the buffer.
    pub(crate) marked: Option<String>,
    /// The windows as last painted, for the input method.
    pub(crate) painted: Vec<Painted>,
    /// The windows area's size and cell width last sent to the core.
    pub(crate) reported: (Pixels, Pixels, Pixels),
}

#[derive(Default)]
pub(crate) struct Stats {
    /// When the last frames were rendered, for the frame rate.
    frames: VecDeque<Instant>,
    /// How long the core took to handle the last message.
    handle: Duration,
    /// How long building the last frame's elements took.
    render: Duration,
    /// How long laying out the last frame's lines took.
    pub(crate) shape: Duration,
    /// How long the last heartbeat took from a Tokio task to the UI thread.
    tick: Duration,
}

impl Editor {
    /// `channel` is the services channel, made first so `tracing` can log into it.
    pub(crate) fn new(
        core: Core,
        args: Args,
        (tx, rx): (mpsc::Sender<Msg>, mpsc::Receiver<Msg>),
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        // Closing the window quits as `C-x C-c` does, so unsaved buffers are offered first.
        let this = cx.weak_entity();
        window.on_window_should_close(cx, move |_, cx| {
            _ = this.update(cx, |editor, cx| editor.handle(Msg::Quit, cx));
            false
        });
        Self::drain(rx, cx);
        if args.debug {
            spawn_heartbeat(cx);
        }
        let reports = tx.clone();
        let watcher = watcher::Watcher::spawn(move |msg| _ = reports.blocking_send(msg));
        // The launch's messages ran before there was a watcher to send their watch set to.
        watcher.set(core.watch_set());
        Editor {
            core,
            benchmark: None,
            focus: cx.focus_handle(),
            tx,
            timer: None,
            config_dir: args.config_dir,
            grep: grep::Grep::default(),
            terminals: terminal::Terminals::new(args.zsh_dir),
            agents: agents::Agents::default(),
            watcher,
            recovery: args.recovery_dir.map(recovery::spawn),
            debug: args.debug.then(Stats::default),
            marked: None,
            painted: Vec::new(),
            reported: Default::default(),
        }
    }

    fn drain(mut rx: mpsc::Receiver<Msg>, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            while let Some(msg) = rx.recv().await {
                if this
                    .update(cx, |editor, cx| editor.handle(msg, cx))
                    .is_err()
                {
                    break;
                }
                // `recv` never pends while messages queue, so a flood would starve keys and frames.
                tokio::task::yield_now().await;
            }
        })
        .detach();
    }

    /// Every message reaches the core here, so the timer is always re-armed.
    pub(crate) fn handle(&mut self, msg: Msg, cx: &mut Context<Self>) {
        let start = Instant::now();
        if let Some(benchmark) = &mut self.benchmark {
            benchmark.output(&msg, start);
        }
        let effects = self.core.handle(start, msg);
        if let Some(debug) = &mut self.debug {
            debug.handle = start.elapsed();
        }
        for effect in effects {
            match effect {
                Effect::Terminal(op) => {
                    if let Some(rx) = self.terminals.run(op) {
                        Self::drain(rx, cx)
                    }
                }
                Effect::Agent(op) => {
                    if let Some(rx) = self.agents.run(op) {
                        Self::drain(rx, cx)
                    }
                }
                Effect::File(op) => {
                    let tx = self.tx.clone();
                    tokio::task::spawn_blocking(move || _ = tx.blocking_send(files::run(op)));
                }
                Effect::Grep(op) => self.grep.run(op, self.tx.clone()),
                Effect::Watch(set) => self.watcher.set(set),
                Effect::SetClipboard(text) => {
                    cx.write_to_clipboard(ClipboardItem::new_string(text))
                }
                Effect::Recovery(op) => {
                    if let Some((tx, _)) = &self.recovery {
                        _ = tx.send(op);
                    }
                }
                Effect::Quit => {
                    // Lets queued recovery deletes finish before the process exits.
                    if let Some((tx, thread)) = self.recovery.take() {
                        drop(tx);
                        _ = thread.join();
                    }
                    self.terminals.kill_all();
                    self.agents.kill_all();
                    cx.quit()
                }
                Effect::CaptureEnvironment {
                    project,
                    request,
                    dir,
                } => {
                    let tx = self.tx.clone();
                    tokio::spawn(async move {
                        let mut result = environment::run(&dir).await;
                        let direnv = match &mut result {
                            Ok(vars) => {
                                environment::direnv(vars, &dir, environment::DIRENV_TIMEOUT)
                                    .await
                                    .err()
                            }
                            Err(_) => None,
                        };
                        _ = (tx.send(Msg::EnvironmentCaptured {
                            project,
                            request,
                            result,
                            direnv,
                        }))
                        .await;
                    });
                }
                Effect::LoadConfiguration { request } => {
                    let dir = self.config_dir.clone();
                    let tx = self.tx.clone();
                    tokio::task::spawn_blocking(move || {
                        _ = tx.blocking_send(configuration::load(dir.as_deref(), request));
                    });
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

    /// The buffer font and the width of one of its cells.
    pub(crate) fn font(&self, window: &Window) -> (Font, Pixels) {
        let settings = self.core.settings();
        let font = font(settings.font_family.clone().unwrap_or(FONT.into()));
        let size = px(settings.font_size);
        let text = window.text_system();
        let cell = text.ch_advance(text.resolve_font(&font), size);
        (font, cell.unwrap_or(size * 0.6))
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
        let frame = self.core.frame();
        let settings = self.core.settings();
        let theme = &settings.theme;
        let line_height = px(settings.line_height());
        window.set_window_title(&frame.title);
        let line = || div().h(line_height).whitespace_nowrap().overflow_hidden();
        let hint = frame.hint.map(|hint| {
            // Keys right-aligned so the colons line up, as Emacs's which-key.
            let keys = (hint.entries.iter()).map(|(key, _)| key.chars().count());
            let keys = keys.max().unwrap_or(0);
            let entries: Vec<String> = (hint.entries.iter())
                .map(|(key, what)| format!("{key:>keys$} : {what}"))
                .collect();
            let width = entries.iter().map(|e| e.chars().count()).max().unwrap_or(0) + 2;
            let column = cell * width as f32;
            div()
                .flex()
                .flex_wrap()
                .child(line().w_full().child(format!("{}-", hint.keys)))
                .children(entries.into_iter().map(|e| line().w(column).child(e)))
        });
        let message = frame.message.unwrap_or_default();
        let echo = match frame.minibuffer {
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
                let (x, width, under) = block(&shaped, byte, cell);
                let cursor = div()
                    .absolute()
                    .top_0()
                    .left(x)
                    .w(width)
                    .h(line_height)
                    .bg(rgb(theme.cursor))
                    .text_color(rgb(theme.background))
                    .child(under.to_string());
                let prompt = line().relative().child(text).child(cursor);
                let padding = (mb.candidates.len()..mb.rows).map(|_| line());
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
                div().child(prompt).children(candidates).children(padding)
            }
            None => div().child(line().child(message)),
        };
        let root = div()
            .track_focus(&self.focus)
            .on_key_down(cx.listener(Self::on_key_down))
            .size_full()
            // A gap like Emacs's 8px fringes, so the rounded window corners never clip text.
            .px(px(8.))
            .pb(px(2.))
            .overflow_hidden()
            .flex()
            .flex_col()
            .bg(rgb(theme.background))
            .text_color(rgb(theme.foreground))
            .font_family(font.family.clone())
            .text_size(px(settings.font_size))
            .children(stats.map(|s| line().child(s)))
            .child(div().flex_1().flex().overflow_hidden().child(FrameElement {
                editor: cx.entity(),
                windows: frame.windows,
            }))
            .children(hint)
            .child(echo);
        if let Some(debug) = &mut self.debug {
            debug.render = start.elapsed();
        }
        root
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aiditor_core::command::{ArgSpec, Command};
    use aiditor_core::key::parse_keys;
    use aiditor_core::key::{Key, KeyCode};
    use aiditor_core::keymap::{self, Binding, Keymap};
    use gpui::{Bounds, EntityInputHandler, ShapedLine};
    use gpui::{TestAppContext, VisualTestContext};

    fn editor<'a>(
        cx: &'a mut TestAppContext,
        text: &str,
    ) -> (gpui::Entity<Editor>, &'a mut VisualTestContext) {
        let (editor, cx) = cx.add_window_view(|window, cx| {
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join("t.txt");
            std::fs::write(&path, text).unwrap();
            let mut core = Core::default();
            core.open(&path, files::read(&path).unwrap());
            cx.on_release(move |_, _| drop(dir)).detach();
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
            KeyCode::Home => "home".into(),
            KeyCode::End => "end".into(),
            KeyCode::PageUp => "pageup".into(),
            KeyCode::PageDown => "pagedown".into(),
            KeyCode::Delete => "delete".into(),
            KeyCode::F(n) => format!("f{n}"),
        }
    }

    #[gpui::test]
    fn every_bundled_binding_reaches_the_core(cx: &mut TestAppContext) {
        // `C-x C-s` hands the save to Tokio's blocking pool.
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let _guard = runtime.enter();
        let (editor, cx) = editor(cx, "");
        let (maps, _) = keymap::parse(&keymap::BUNDLED, None, |_| Some("any")).unwrap();
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
        // Release the async receiver before leaving the Tokio runtime.
        cx.update(|window, _| window.remove_window());
        drop(editor);
        cx.run_until_parked();
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
    fn the_window_title_follows_the_current_workspace(cx: &mut TestAppContext) {
        let (editor, cx) = editor(cx, "");
        let shown = cx.window_title().unwrap();
        assert!(shown.starts_with("t.txt | "), "{shown}");
        editor.update(cx, |e, cx| {
            let project =
                aiditor_core::project::Project::parse("name = 'other'\n[primary]\npath = '/other'")
                    .unwrap();
            e.core.handle(
                Instant::now(),
                Msg::ProjectsLoaded {
                    dir: None,
                    projects: vec![Ok(aiditor_core::project::Loaded {
                        project,
                        file: None,
                    })],
                },
            );
            e.core
                .new_workspace("w2", e.core.project_id("other").unwrap());
            cx.notify();
        });
        cx.run_until_parked();
        let shown = cx.window_title();
        assert_eq!(shown.as_deref(), Some("*scratch*<w2> | w2 | other"));
    }

    #[gpui::test]
    fn closing_the_window_offers_to_save(cx: &mut TestAppContext) {
        let (editor, cx) = editor(cx, "");
        cx.simulate_input("x");
        let path = editor.read_with(cx, |e, _| e.core.current().file.clone().unwrap());
        assert!(!cx.simulate_close(), "the window stays open");
        let prompt = editor.update(cx, |e, _| e.core.frame().minibuffer.map(|mb| mb.prompt));
        assert_eq!(prompt, Some(format!("Save file {}? ", path.display())));
    }

    #[gpui::test]
    fn startup_indexes_before_watching_and_applies_ignores(cx: &mut TestAppContext) {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let _guard = runtime.enter();
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        std::fs::create_dir(root.join("ignored")).unwrap();
        std::fs::write(root.join("x"), "").unwrap();
        std::fs::write(root.join("ignored/y"), "").unwrap();
        std::fs::write(root.join(".ignore"), "ignored/\n.ignore\n").unwrap();
        let mut core = Core::default();
        core.handle(
            std::time::Instant::now(),
            Msg::SettingsLoaded(Ok(Some("max_watched_files = 1".into()))),
        );
        core.launch(root, root);
        let channel = aiditor_services::channel();
        channel.0.try_send(Msg::Recover(Vec::new())).unwrap();
        let (editor, cx) = cx
            .add_window_view(|window, cx| Editor::new(core, Args::default(), channel, window, cx));
        for _ in 0..500 {
            cx.run_until_parked();
            if !editor.read_with(cx, |e, _| e.core.watch_set().roots.is_empty()) {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        assert_eq!(
            editor.read_with(cx, |e, _| e.core.watch_set().roots),
            [(root.to_path_buf(), Vec::<String>::new())]
        );
        editor.update(cx, |e, cx| {
            e.handle(
                Msg::SettingsLoaded(Ok(Some("max_watched_files = 0".into()))),
                cx,
            )
        });
        let message = editor.read_with(cx, |e, _| e.core.message().unwrap().to_owned());
        assert!(message.contains("files = 1, directories = 1; max_watched_files = 0 exceeded"));
        cx.update(|window, _| window.remove_window());
        drop(editor);
        cx.run_until_parked();
    }

    #[gpui::test]
    fn c_x_c_r_reloads_projects(cx: &mut TestAppContext) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let _guard = rt.enter();
        let dir = tempfile::tempdir().unwrap();
        let args = Args {
            config_dir: Some(dir.path().to_owned()),
            ..Args::default()
        };
        let (editor, cx) = cx.add_window_view(|window, cx| {
            Editor::new(
                Core::default(),
                args,
                aiditor_services::channel(),
                window,
                cx,
            )
        });
        cx.update(|window, cx| window.focus(&editor.read(cx).focus));
        let projects = dir.path().join("projects");
        std::fs::create_dir(&projects).unwrap();
        let text = format!(
            "name = \"p\"\n[primary]\npath = \"{}\"",
            dir.path().display()
        );
        std::fs::write(projects.join("p.toml"), text).unwrap();
        std::fs::write(dir.path().join("settings.toml"), "font_size = 18").unwrap();
        cx.simulate_keystrokes("ctrl-x ctrl-r");
        cx.simulate_input("typing");
        assert_eq!(editor.read_with(cx, |e, _| e.core.text()), "typing");
        for _ in 0..500 {
            cx.run_until_parked();
            if editor.read_with(cx, |e, _| e.core.settings().font_size == 18.) {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        assert_eq!(
            editor.read_with(cx, |e, _| e.core.settings().font_size),
            18.
        );
        let names = editor.read_with(cx, |e, _| {
            e.core
                .projects()
                .iter()
                .map(|p| p.name.clone())
                .collect::<Vec<_>>()
        });
        assert_eq!(names[1..], ["p"], "after the launch `$HOME` project");
        cx.update(|window, _| window.remove_window());
        drop(editor);
        cx.run_until_parked();
    }

    #[gpui::test]
    fn integrated_finders_visits_saves_and_directory_recovery(cx: &mut TestAppContext) {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .max_blocking_threads(1)
            .enable_all()
            .build()
            .unwrap();
        let _guard = runtime.enter();
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap().join("repo");
        std::fs::create_dir_all(root.join("sub")).unwrap();
        for name in ["a", "b", "c"] {
            std::fs::write(root.join("sub").join(name), format!("needle {name}\n")).unwrap();
        }
        let mut core = Core::default();
        core.launch(&root, &root);
        let first = core.workspace();
        let project = core.project_id("repo").unwrap();
        let second = core.new_workspace("second", project);
        core.switch_workspace(first).unwrap();
        let (editor, cx) = cx.add_window_view(|window, cx| {
            Editor::new(
                core,
                Args::default(),
                aiditor_services::channel(),
                window,
                cx,
            )
        });
        cx.update(|window, cx| window.focus(&editor.read(cx).focus));
        let wait = |cx: &mut VisualTestContext, label: &str, done: fn(&mut Core) -> bool| {
            for _ in 0..1000 {
                cx.executor().advance_clock(Duration::from_millis(20));
                cx.run_until_parked();
                if editor.update(cx, |e, cx| {
                    e.handle(Msg::Tick, cx);
                    done(&mut e.core)
                }) {
                    return;
                }
                std::thread::sleep(Duration::from_millis(5));
            }
            panic!("{label}: {:?}", editor.update(cx, |e, _| e.core.frame()));
        };
        let hold = || {
            let (started, ready) = std::sync::mpsc::channel();
            let (release, held) = std::sync::mpsc::channel();
            runtime.spawn_blocking(move || {
                started.send(()).unwrap();
                _ = held.recv();
            });
            ready.recv_timeout(Duration::from_secs(5)).unwrap();
            release
        };
        wait(cx, "initial index", |core| {
            !core.watch_set().roots.is_empty()
        });
        let release = hold();
        cx.simulate_keystrokes("ctrl-x p f");
        wait(cx, "finder candidates", |core| {
            core.frame()
                .minibuffer
                .is_some_and(|m| m.candidates.len() == 3)
        });
        cx.simulate_keystrokes("ctrl-n ctrl-n");
        assert_eq!(
            editor.update(cx, |e, _| e.core.frame().minibuffer.unwrap().selected),
            Some(2)
        );
        release.send(()).unwrap();
        wait(cx, "latest finder preview", |core| {
            core.frame().windows[0].display.mode_line.contains(" c ")
        });
        cx.simulate_keystrokes("enter");
        wait(cx, "ordinary visit", |core| {
            core.current()
                .file
                .as_ref()
                .is_some_and(|p| p.ends_with("sub/c"))
        });
        assert_eq!(editor.read_with(cx, |e, _| e.core.text()), "needle c\n");

        let release = hold();
        cx.simulate_keystrokes("ctrl-x p f");
        wait(cx, "second finder", |core| {
            core.frame()
                .minibuffer
                .is_some_and(|m| m.candidates.len() == 3)
        });
        cx.simulate_keystrokes("enter ctrl-x t o");
        cx.simulate_input("other workspace");
        release.send(()).unwrap();
        runtime.block_on(runtime.spawn_blocking(|| {})).unwrap();
        cx.run_until_parked();
        assert_eq!(
            editor.read_with(cx, |e, _| (e.core.workspace(), e.core.text())),
            (second, "other workspace".into())
        );
        editor.update(cx, |e, cx| {
            e.core.switch_workspace(first).unwrap();
            e.handle(Msg::Tick, cx);
        });
        cx.simulate_keystrokes("ctrl-x p g");
        cx.simulate_input("needle");
        wait(cx, "grep hits", |core| {
            core.frame()
                .minibuffer
                .is_some_and(|m| m.candidates.len() == 3)
        });
        cx.simulate_keystrokes("ctrl-n ctrl-n enter");
        wait(cx, "accepted grep hit", |core| {
            core.current()
                .file
                .as_ref()
                .is_some_and(|p| p.ends_with("sub/c"))
                && core.point() == 0
        });

        for _ in 0..50 {
            std::fs::write(root.join("sub/c"), "ready\n").unwrap();
            std::thread::sleep(Duration::from_millis(100));
            cx.run_until_parked();
            if editor.read_with(cx, |e, _| e.core.text() == "ready\n") {
                break;
            }
        }
        wait(cx, "native watcher ready", |core| core.text() == "ready\n");

        std::fs::rename(root.join("sub"), dir.path().join("old-sub")).unwrap();
        std::fs::create_dir(root.join("sub")).unwrap();
        std::fs::write(root.join("sub/c"), "replacement\n").unwrap();
        wait(cx, "replacement directory", |core| {
            core.text() == "replacement\n"
        });
        std::fs::write(root.join("sub/c"), "later\n").unwrap();
        wait(cx, "later notification", |core| core.text() == "later\n");

        editor.update(cx, |e, cx| {
            e.core.switch_workspace(second).unwrap();
            let path = root.join("sub/c");
            e.core.open(&path, files::read(&path).unwrap());
            e.core.switch_workspace(first).unwrap();
            e.handle(Msg::Tick, cx);
        });
        let release = hold();
        cx.simulate_input("first ");
        cx.simulate_keystrokes("ctrl-x ctrl-s ctrl-x t o");
        cx.simulate_input("second ");
        cx.simulate_keystrokes("ctrl-x ctrl-s");
        release.send(()).unwrap();
        wait(cx, "conflicting save replies", |core| {
            core.current().conflict.is_some()
        });
        assert_eq!(
            std::fs::read_to_string(root.join("sub/c")).unwrap(),
            "first later\n"
        );
        assert_eq!(editor.read_with(cx, |e, _| e.core.text()), "second later\n");
        assert!(editor.read_with(cx, |e, _| e.core.current().modified()));
        editor.update(cx, |e, _| e.core.switch_workspace(first).unwrap());
        assert_eq!(
            editor.read_with(cx, |e, _| (e.core.text(), e.core.current().modified())),
            ("first later\n".into(), false)
        );
        cx.update(|window, _| window.remove_window());
        drop(editor);
        cx.run_until_parked();
    }

    #[gpui::test]
    fn recovery_files_are_written_and_removed_on_quit(cx: &mut TestAppContext) {
        let dir = tempfile::tempdir().unwrap();
        let args = Args {
            recovery_dir: Some(dir.path().to_owned()),
            ..Args::default()
        };
        let channel = aiditor_services::channel();
        let mut core = Core::default();
        let source = tempfile::tempdir().unwrap();
        let path = source.path().join("t.txt");
        std::fs::write(&path, "").unwrap();
        core.open(&path, files::read(&path).unwrap());
        let (editor, cx) =
            cx.add_window_view(|window, cx| Editor::new(core, args, channel, window, cx));
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
        editor.update(cx, |e, cx| e.handle(Msg::Quit, cx));
        cx.simulate_input("no");
        cx.simulate_keystrokes("enter");
        cx.simulate_input("yes");
        cx.simulate_keystrokes("enter");
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
        let prompt = editor.update(cx, |e, _| e.core.frame().minibuffer.map(|mb| mb.prompt));
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
            let frame = e.core.frame();
            echo_line(
                &frame.minibuffer.unwrap(),
                &frame.message.unwrap_or_default(),
            )
        });
        let text = "0/0 M-x abc [M-z is undefined]".to_string();
        assert_eq!(line, (text, "0/0 M-x a".len()));
    }

    #[gpui::test]
    fn m_x_runs_a_command_by_name(cx: &mut TestAppContext) {
        let (editor, cx) = editor(cx, "abc");
        cx.simulate_keystrokes("alt-x");
        cx.simulate_input("forward-char");
        cx.run_until_parked();
        let prompt = editor.update(cx, |e, _| e.core.frame().minibuffer.map(|mb| mb.prompt));
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
        cx.run_until_parked();
        // The matcher's threads may still be running; the core's repoll timer catches up.
        let listed = |cx: &mut VisualTestContext| {
            editor.update(cx, |e, _| e.core.frame().minibuffer.unwrap().candidates)
        };
        let wait = |cx: &mut VisualTestContext, n: usize| {
            for _ in 0..100 {
                if listed(cx).len() == n {
                    break;
                }
                std::thread::sleep(Duration::from_millis(5));
                cx.executor().advance_clock(Duration::from_millis(20));
                cx.run_until_parked();
            }
        };
        wait(cx, 2);
        let windows_size = |cx: &mut VisualTestContext| editor.read_with(cx, |e, _| e.reported);
        let before = windows_size(cx);
        cx.simulate_input("an");
        cx.run_until_parked();
        wait(cx, 1);
        assert_eq!(listed(cx), ["banana"]);
        assert_eq!(windows_size(cx), before, "the list keeps its height");
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
    fn copy_and_paste_go_through_the_clipboard(cx: &mut TestAppContext) {
        let (editor, cx) = editor(cx, "ab");
        let (copy, paste) = if cfg!(target_os = "macos") {
            ("cmd-c", "cmd-v")
        } else {
            ("ctrl-shift-c", "ctrl-shift-v")
        };
        cx.simulate_keystrokes(&format!("ctrl-space ctrl-e {copy}"));
        let copied = cx.read_from_clipboard().and_then(|item| item.text());
        assert_eq!(copied.as_deref(), Some("ab"));
        cx.write_to_clipboard(ClipboardItem::new_string("xyz".into()));
        cx.simulate_keystrokes(paste);
        assert_eq!(editor.read_with(cx, |e, _| e.core.text()), "abxyz");
    }

    /// Window `i`'s bounds and its rows' laid-out text, as last painted.
    fn painted(
        editor: &gpui::Entity<Editor>,
        cx: &mut VisualTestContext,
        i: usize,
    ) -> (Bounds<Pixels>, Vec<ShapedLine>) {
        editor.read_with(cx, |e, _| {
            let p = &e.painted[i];
            let rows = p.rows.iter().map(|r| r.text.clone());
            (p.bounds, rows.collect())
        })
    }

    #[gpui::test]
    fn a_full_channel_lets_other_ui_work_run(cx: &mut TestAppContext) {
        let (editor, cx) = editor(cx, "");
        let (tx, rx) = mpsc::channel(1000);
        for _ in 0..1000 {
            tx.try_send(Msg::Tick).unwrap();
        }
        editor.update(cx, |_, cx| Editor::drain(rx, cx));
        let seen = std::rc::Rc::new(std::cell::Cell::new(0));
        let seen2 = seen.clone();
        cx.update(|_, cx| {
            cx.spawn(async move |_| {
                while tx.capacity() == 0 {
                    tokio::task::yield_now().await;
                }
                seen2.set(tx.capacity());
            })
            .detach()
        });
        cx.run_until_parked();
        assert!(
            seen.get() < 1000,
            "drained {} before other work ran",
            seen.get()
        );
    }

    #[gpui::test]
    fn terminal_cells_sit_on_the_grid(cx: &mut TestAppContext) {
        let (editor, cx) = editor(cx, "");
        editor.update(cx, |e, cx| {
            let (id, _) = e.core.create_terminal("term", 3, 20);
            let bytes = "日本x\r\nabcde".as_bytes().to_vec();
            e.handle(Msg::TerminalOutput { id, bytes }, cx);
        });
        cx.simulate_keystrokes("ctrl-x b");
        cx.simulate_input("term");
        cx.simulate_keystrokes("enter ctrl-x 2");
        cx.run_until_parked();
        let (x, e, hollow) = editor.read_with(cx, |ed, _| {
            let cells = &ed.painted[0].cells;
            let at = |text: &str| cells.iter().find(|c| c.text.text.as_ref() == text).unwrap();
            let abcde = at("abcde");
            let e = abcde.origin.x + abcde.text.x_for_index(4);
            let hollow = ed.painted.iter().map(|p| p.hollow).collect::<Vec<_>>();
            (at("x").origin.x, e, hollow)
        });
        assert!((x - e).abs() < px(0.01), "x at {x:?}, e at {e:?}");
        assert_eq!(
            hollow,
            [false, true],
            "the unselected window's cursor is hollow"
        );
    }

    #[gpui::test]
    fn the_cursor_is_drawn_over_the_whole_line(cx: &mut TestAppContext) {
        let (editor, cx) = editor(cx, "hello world");
        let cursor = |cx: &mut VisualTestContext| {
            let (bounds, rows) = painted(&editor, cx, 0);
            let x = editor.read_with(cx, |e, _| e.painted[0].cursor.unwrap().left());
            (rows[0].clone(), x - bounds.left())
        };
        for col in [0, 3, 7] {
            cx.simulate_keystrokes(&format!("ctrl-a ctrl-u {col} ctrl-f"));
            let (line, x) = cursor(cx);
            assert_eq!(line.text.as_ref(), "hello world", "shaped whole at {col}");
            assert!(
                (x - line.x_for_index(col)).abs() < px(0.01),
                "cursor at {col}"
            );
        }
        assert!(cursor(cx).1 > px(0.), "the test platform lays text out");
        let width = editor.read_with(cx, |e, _| e.painted[0].cursor.unwrap().size.width);
        assert!(width > px(2.), "a block over the char at point");
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
}
