pub(crate) mod benchmark;

use std::collections::VecDeque;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use aiditor_core::buffer::Face;
use aiditor_core::display::TEXT_INSET;
use aiditor_core::settings::FaceName;
use aiditor_core::{Core, Effect, Msg, RecoveryOp};
use aiditor_services::{
    agents, clock, configuration, environment, files, grep, recovery, terminal, watcher,
};
use gpui::{
    ClipboardItem, Context, FocusHandle, Font, FontFeatures, KeyDownEvent, Pixels, StyledText,
    Task, Window, div, font, prelude::*, px, rgb,
};
use tokio::sync::mpsc;

use crate::element::{
    FrameElement, Painted, block, completion_runs, echo_line, grep_runs, text_run,
};
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
    clock: String,
    _clock_task: Task<()>,
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
        let (clock, clock_task) = Self::start_clock(clock::read, cx);
        Editor {
            core,
            benchmark: None,
            focus: cx.focus_handle(),
            tx,
            timer: None,
            clock,
            _clock_task: clock_task,
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

    fn start_clock(
        mut read: impl FnMut() -> (String, Duration) + 'static,
        cx: &mut Context<Self>,
    ) -> (String, Task<()>) {
        let (text, mut delay) = read();
        let task = cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(delay).await;
                let (text, next) = read();
                delay = next;
                if this
                    .update(cx, |editor, cx| {
                        editor.clock = text;
                        cx.notify();
                    })
                    .is_err()
                {
                    break;
                }
            }
        });
        (text, task)
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
        let mut font = font(settings.font_family.clone().unwrap_or(FONT.into()));
        // Ligatures hide which characters are really there.
        font.features = FontFeatures::disable_ligatures();
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
        let cursor_fg = theme
            .face(FaceName::Cursor)
            .fg
            .unwrap_or(theme.background());
        let line_height = px(settings.line_height());
        window.set_window_title(&frame.title);
        let mode_line_height = px(settings.mode_line_height());
        let inset = px(TEXT_INSET);
        let row = |height: Pixels| {
            (div().h(height).line_height(height))
                .whitespace_nowrap()
                .overflow_hidden()
        };
        let line = || row(line_height);
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
                .px(inset)
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
                    .debug_selector(|| "minibuffer-cursor".into())
                    .absolute()
                    .top_0()
                    .left(x)
                    .w(width)
                    .h(mode_line_height)
                    .bg(rgb(theme.cursor()))
                    .text_color(rgb(cursor_fg))
                    .child(under.to_string());
                let status = mb.status.map(|status| {
                    div()
                        .debug_selector(|| "grep-status".into())
                        .absolute()
                        .top_0()
                        .right_0()
                        .bg(rgb(theme.background()))
                        .text_color(rgb(theme
                            .face(FaceName::Annotation)
                            .fg
                            .unwrap_or(theme.foreground())))
                        .child(status)
                });
                let prompt = row(mode_line_height)
                    .debug_selector(|| "minibuffer-prompt".into())
                    .relative()
                    .child(text)
                    .child(cursor)
                    .children(status);
                let prompt = div().px(inset).child(prompt);
                let padding = (mb.candidates.len() + mb.groups.len()..mb.rows).map(|_| line());
                let selected = theme
                    .face(FaceName::CompletionSelected)
                    .bg
                    .unwrap_or(theme.background());
                let grep = !mb.groups.is_empty();
                let mut groups = mb.groups.into_iter().peekable();
                let rows = mb
                    .candidates
                    .into_iter()
                    .zip(mb.highlights)
                    .zip(mb.annotations)
                    .enumerate();
                let candidates = rows.flat_map(|(i, ((c, highlights), note))| {
                    let header = if groups.peek().is_some_and(|(at, _)| *at == i) {
                        let (_, label) = groups.next().unwrap();
                        let rule = || {
                            div().h(px(1.)).bg(rgb(theme
                                .face(FaceName::Separator)
                                .fg
                                .unwrap_or(theme.foreground())))
                        };
                        let run = text_run(
                            label.len(),
                            theme.face(FaceName::GrepFile),
                            false,
                            &font,
                            theme,
                        );
                        Some(
                            line()
                                .debug_selector(|| format!("grep-header-{i}"))
                                .flex()
                                .items_center()
                                .gap(cell)
                                .px(inset)
                                .child(
                                    rule()
                                        .w(cell * 2.)
                                        .flex_shrink_0()
                                        .debug_selector(|| format!("grep-rule-before-{i}")),
                                )
                                .child(
                                    div()
                                        .min_w_0()
                                        .overflow_hidden()
                                        .child(StyledText::new(label).with_runs(vec![run])),
                                )
                                .child(
                                    rule()
                                        .flex_1()
                                        .debug_selector(|| format!("grep-rule-after-{i}")),
                                ),
                        )
                    } else {
                        None
                    };
                    let runs = if grep {
                        grep_runs(&c, &font, theme)
                    } else {
                        completion_runs(c.len(), &highlights, &font, theme)
                    };
                    let text = StyledText::new(c).with_runs(runs);
                    let note =
                        (!note.is_empty()).then(|| div().ml(cell * 2.).opacity(0.6).child(note));
                    let candidate = line()
                        .debug_selector(|| format!("completion-{i}"))
                        .flex()
                        .px(inset)
                        .child(text)
                        .children(note);
                    let candidate = match Some(i) == mb.selected {
                        true => candidate.bg(rgb(selected)),
                        false => candidate,
                    };
                    header.into_iter().chain(Some(candidate))
                });
                div().child(prompt).children(candidates).children(padding)
            }
            None => div().child(row(mode_line_height).px(inset).child(message)),
        };
        let bar = theme.face(FaceName::WorkspaceBar);
        let workspace_text: String = frame.workspaces.iter().map(|s| s.text.as_str()).collect();
        let runs = frame
            .workspaces
            .iter()
            .map(|s| text_run(s.text.len(), s.face, false, &font, theme))
            .collect();
        let workspaces = StyledText::new(workspace_text).with_runs(runs);
        let workspace_gap = cell * (2. * settings.workspace_font_size() / settings.font_size);
        let root = div()
            .track_focus(&self.focus)
            .on_key_down(cx.listener(Self::on_key_down))
            .size_full()
            .overflow_hidden()
            .flex()
            .flex_col()
            .bg(rgb(theme.background()))
            .text_color(rgb(theme.foreground()))
            .font(font)
            .text_size(px(settings.font_size))
            .children(stats.map(|s| line().px(inset).child(s)))
            .child(
                row(px(settings.workspace_modeline_height()))
                    .flex()
                    .flex_none()
                    .px(inset)
                    .text_size(px(settings.workspace_font_size()))
                    .debug_selector(|| "workspace-modeline".into())
                    .bg(rgb(bar.bg.unwrap_or(theme.background())))
                    .text_color(rgb(bar.fg.unwrap_or(theme.foreground())))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .overflow_hidden()
                            .debug_selector(|| "workspace-entries".into())
                            .child(workspaces),
                    )
                    .child(
                        div()
                            .flex_none()
                            .ml(workspace_gap)
                            .debug_selector(|| "workspace-clock".into())
                            .child(self.clock.clone()),
                    ),
            )
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
            // The fixture has no Tokio runtime for the file's repository lookup.
            core.handle(Instant::now(), Msg::Tick);
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
        let _native = crate::native::lock();
        // `C-x C-s` hands the save to Tokio's blocking pool.
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let _guard = runtime.enter();
        let (editor, cx) = editor(cx, "");
        editor.update(cx, |e, _| {
            let path = e.core.current().file.clone().unwrap();
            e.core.launch(&path, path.parent().unwrap());
        });
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
        let _native = crate::native::lock();
        let (editor, cx) = editor(cx, "");
        cx.simulate_input("aB");
        cx.simulate_keystrokes("ctrl-u 3 x ctrl-x u");
        let text = editor.read_with(cx, |e, _| e.core.text());
        assert_eq!(text, "aB", "C-u 3 x typed xxx and C-x u undid it");
        cx.simulate_keystrokes("ctrl-u 3 x");
        assert_eq!(editor.read_with(cx, |e, _| e.core.text()), "aBxxx");
    }

    #[gpui::test]
    fn swiper_input_produces_match_background_runs_and_cleans_up(cx: &mut TestAppContext) {
        let _native = crate::native::lock();
        let (editor, cx) = editor(cx, "été Foo foo\nfoo bar\n");
        for close in ["ctrl-g", "enter"] {
            cx.simulate_keystrokes("alt-< ctrl-s");
            cx.simulate_input("foo");
            editor.update_in(cx, |e, window, _| {
                let line = e.core.frame().windows.remove(0).display.lines.remove(0);
                let font = e.font(window).0;
                let theme = &e.core.settings().theme;
                let runs: Vec<_> = line
                    .spans
                    .iter()
                    .map(|s| text_run(s.text.len(), s.face, false, &font, theme))
                    .collect();
                assert_eq!(runs.iter().map(|r| r.len).collect::<Vec<_>>(), [6, 3, 1, 3]);
                assert_eq!(
                    runs[1].background_color,
                    theme.face(FaceName::MatchCurrent).bg.map(|c| rgb(c).into())
                );
                assert_eq!(
                    runs[3].background_color,
                    theme.face(FaceName::Match).bg.map(|c| rgb(c).into())
                );
                assert_eq!(runs[1].font.weight, gpui::FontWeight::BOLD);
            });
            cx.run_until_parked();
            assert_eq!(painted(&editor, cx, 0).1[0].text.as_ref(), "été Foo foo");
            cx.simulate_keystrokes("ctrl-n");
            editor.update(cx, |e, _| {
                let line = e.core.frame().windows.remove(0).display.lines.remove(1);
                assert_eq!(line.spans[0].text, "foo");
                assert_eq!(
                    line.spans[0].face.bg,
                    e.core.settings().theme.face(FaceName::MatchCurrent).bg
                );
            });
            cx.simulate_keystrokes(close);
            editor.update(cx, |e, _| {
                assert_eq!(e.core.text(), "été Foo foo\nfoo bar\n");
                for line in e.core.frame().windows.remove(0).display.lines {
                    for span in line.spans {
                        assert_eq!(span.face, Face::default());
                    }
                }
            });
        }
    }

    #[gpui::test]
    fn the_window_title_follows_the_current_workspace(cx: &mut TestAppContext) {
        let _native = crate::native::lock();
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
    fn the_workspace_modeline_sits_above_the_windows(cx: &mut TestAppContext) {
        let (editor, cx) = editor(cx, "");
        cx.simulate_keystrokes("ctrl-x 3");
        let (tops, bar) = editor.read_with(cx, |e, _| {
            let tops: Vec<_> = e.painted.iter().map(|p| p.bounds.origin.y).collect();
            (tops, px(e.core.settings().workspace_modeline_height()))
        });
        assert_eq!(tops.len(), 2);
        for top in tops {
            // Layout rounds to device pixels.
            assert!(
                (top - bar).abs() < px(1.),
                "windows start at {top:?}, the bar is {bar:?}"
            );
        }
        let bar = cx.debug_bounds("workspace-modeline").unwrap();
        let width = cx.update(|window, _| window.viewport_size().width);
        assert_eq!(
            (bar.origin.x, bar.size.width),
            (px(0.), width),
            "no side gap"
        );
        assert_eq!(bar.size.height, px(22.), "13 px type: 18.2 plus 4, rounded");
        let frame = editor.update(cx, |e, _| e.core.frame());
        let workspace = editor.read_with(cx, |e, _| e.core.workspaces()[0].name.clone());
        let project = editor.read_with(cx, |e, _| e.core.projects()[0].name.clone());
        let title = format!("t.txt | {workspace} | {project}");
        assert_eq!(
            frame.title, title,
            "the bar names the title's workspace and project"
        );
    }

    #[gpui::test]
    fn the_workspace_clock_stays_at_the_right_inset_and_wakes_without_keys(
        cx: &mut TestAppContext,
    ) {
        let (editor, cx) = editor(cx, "");
        editor.update(cx, |e, cx| {
            let mut reads = 0;
            let (text, task) = Editor::start_clock(
                move || {
                    reads += 1;
                    match reads {
                        1 => ("20:41".into(), Duration::from_millis(250)),
                        _ => ("20:42".into(), Duration::from_secs(60)),
                    }
                },
                cx,
            );
            e.clock = text;
            e._clock_task = task;
            cx.notify();
        });
        cx.run_until_parked();
        assert_eq!(editor.read_with(cx, |e, _| e.clock.clone()), "20:41");
        cx.executor().advance_clock(Duration::from_millis(249));
        cx.run_until_parked();
        assert_eq!(editor.read_with(cx, |e, _| e.clock.clone()), "20:41");
        cx.executor().advance_clock(Duration::from_millis(1));
        cx.run_until_parked();
        assert_eq!(editor.read_with(cx, |e, _| e.clock.clone()), "20:42");
        let clock = cx.debug_bounds("workspace-clock").unwrap();
        let entries = cx.debug_bounds("workspace-entries").unwrap();
        let width = cx.update(|window, _| window.viewport_size().width);
        assert!((clock.right() - (width - px(TEXT_INSET))).abs() < px(1.));
        assert!(clock.left() > entries.right());
        cx.simulate_keystrokes("ctrl-x t 2 enter");
        cx.simulate_input("second");
        cx.simulate_keystrokes("enter");
        let workspaces = editor.update(cx, |e, _| e.core.frame().workspaces);
        let text: String = workspaces.iter().map(|s| s.text.as_str()).collect();
        assert!(text.ends_with(" › second"), "{text}");
        let clock_after = cx.debug_bounds("workspace-clock").unwrap();
        assert_eq!(clock_after, clock);
    }

    #[gpui::test]
    fn closing_the_window_offers_to_save(cx: &mut TestAppContext) {
        let _native = crate::native::lock();
        let (editor, cx) = editor(cx, "");
        cx.simulate_input("x");
        let path = editor.read_with(cx, |e, _| e.core.current().file.clone().unwrap());
        assert!(!cx.simulate_close(), "the window stays open");
        let prompt = editor.update(cx, |e, _| e.core.frame().minibuffer.map(|mb| mb.prompt));
        assert_eq!(prompt, Some(format!("Save file {}? ", path.display())));
    }

    #[gpui::test]
    fn startup_indexes_before_watching_and_applies_ignores(cx: &mut TestAppContext) {
        let _native = crate::native::lock();
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
        let _native = crate::native::lock();
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
        let _native = crate::native::lock();
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
            core.frame().windows[0]
                .display
                .mode_line
                .text()
                .contains("sub/c ")
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
        let target = editor
            .update(cx, |e, _| {
                e.core
                    .frame()
                    .minibuffer
                    .unwrap()
                    .candidates
                    .iter()
                    .position(|row| row == "  1: needle c")
            })
            .unwrap();
        for _ in 0..target {
            cx.simulate_keystrokes("ctrl-n");
        }
        cx.simulate_keystrokes("enter");
        wait(cx, "accepted grep hit", |core| {
            core.current()
                .file
                .as_ref()
                .is_some_and(|p| p.ends_with("sub/c"))
                && core.point() == 0
        });

        let deadline = Instant::now() + Duration::from_secs(30);
        loop {
            assert!(Instant::now() < deadline, "native watcher never ready");
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
        let _native = crate::native::lock();
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
        // No Tokio runtime here for the file's repository lookup.
        core.handle(Instant::now(), Msg::Tick);
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
        let _native = crate::native::lock();
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
        let _native = crate::native::lock();
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
        let text = "   0/0 M-x abc [M-z is undefined]".to_string();
        assert_eq!(line, (text, "   0/0 M-x a".len()));
    }

    #[gpui::test]
    fn multi_root_grep_prioritizes_the_current_file_and_prefix_selects_one_root(
        cx: &mut TestAppContext,
    ) {
        let _native = crate::native::lock();
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let _guard = runtime.enter();
        let dir = tempfile::tempdir().unwrap();
        let base = dir.path().canonicalize().unwrap();
        for name in ["app", "lib", "docs"] {
            std::fs::create_dir(base.join(name)).unwrap();
            std::fs::write(base.join(name).join("a"), format!("needle {name}\n")).unwrap();
        }
        let project = aiditor_core::project::Project::parse(&format!(
            "name = 'multi'\n[primary]\npath = '{}'\n[[roots]]\npath = '{}'\n[[roots]]\npath = '{}'\n",
            base.join("app").display(), base.join("lib").display(), base.join("docs").display()
        )).unwrap();
        let mut core = Core::default();
        core.handle(
            Instant::now(),
            Msg::ProjectsLoaded {
                dir: None,
                projects: vec![Ok(aiditor_core::project::Loaded {
                    project,
                    file: None,
                })],
            },
        );
        core.new_workspace("multi", core.project_id("multi").unwrap());
        let file = base.join("lib/a");
        core.open(&file, files::read(&file).unwrap());
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
        let wait = |cx: &mut VisualTestContext, count| {
            for _ in 0..1000 {
                cx.executor().advance_clock(Duration::from_millis(20));
                cx.run_until_parked();
                if editor.update(cx, |e, cx| {
                    e.handle(Msg::Tick, cx);
                    e.core.frame().minibuffer.is_some_and(|mb| {
                        mb.candidates.len() == count && mb.status != Some("[searching]")
                    })
                }) {
                    return;
                }
                std::thread::sleep(Duration::from_millis(5));
            }
            panic!(
                "grep did not finish: {:?}",
                editor.update(cx, |e, _| e.core.frame())
            );
        };
        let candidates = |cx: &mut VisualTestContext| {
            editor.update(cx, |e, _| e.core.frame().minibuffer.unwrap().candidates)
        };
        cx.simulate_keystrokes("ctrl-x p g");
        cx.simulate_input("needle");
        wait(cx, 3);
        assert_eq!(
            candidates(cx),
            ["  1: needle lib", "  1: needle app", "  1: needle docs"]
        );
        editor.update_in(cx, |e, window, _| {
            let mb = e.core.frame().minibuffer.unwrap();
            assert_eq!(
                mb.groups,
                [
                    (0, "lib/a".into()),
                    (1, "app/a".into()),
                    (2, "docs/a".into())
                ]
            );
            assert_eq!((mb.rows, mb.count.trim()), (6, "1/3"));
            let font = e.font(window).0;
            let theme = &e.core.settings().theme;
            let runs = grep_runs(&mb.candidates[0], &font, theme);
            assert_eq!(runs[0].len, 3);
            assert_eq!(
                runs[0].color,
                rgb(theme.face(FaceName::GrepLineNumber).fg.unwrap()).into()
            );
            assert_eq!(runs[1].color, rgb(theme.foreground()).into());
            assert_eq!(
                runs.iter().map(|r| r.len).sum::<usize>(),
                mb.candidates[0].len()
            );
        });
        cx.run_until_parked();
        let header = cx.debug_bounds("grep-header-0").unwrap();
        let hit = cx.debug_bounds("completion-0").unwrap();
        let next = cx.debug_bounds("grep-header-1").unwrap();
        assert_eq!(header.bottom(), hit.top());
        assert_eq!(hit.bottom(), next.top());
        for selector in ["grep-rule-before-0", "grep-rule-after-0"] {
            let rule = cx.debug_bounds(selector).unwrap();
            assert_eq!(rule.size.height, px(1.));
            assert!(rule.size.width > px(0.));
        }
        cx.simulate_keystrokes("ctrl-n");
        editor.update(cx, |e, _| {
            let mb = e.core.frame().minibuffer.unwrap();
            assert_eq!((mb.selected, mb.count.trim()), (Some(1), "2/3"));
        });
        cx.simulate_keystrokes("ctrl-p");
        for _ in 0..1000 {
            let ready = editor.update(cx, |e, _| {
                let line = e.core.frame().windows.remove(0).display.lines.remove(0);
                line.spans.first().is_some_and(|span| {
                    span.text == "needle"
                        && span.face.bg == e.core.settings().theme.face(FaceName::MatchCurrent).bg
                })
            });
            if ready {
                break;
            }
            cx.run_until_parked();
            std::thread::sleep(Duration::from_millis(5));
        }
        editor.update(cx, |e, _| {
            let line = e.core.frame().windows.remove(0).display.lines.remove(0);
            assert_eq!(line.spans[0].text, "needle");
            assert_eq!(
                line.spans[0].face.bg,
                e.core.settings().theme.face(FaceName::MatchCurrent).bg
            );
        });
        cx.simulate_keystrokes("ctrl-g ctrl-u ctrl-x p g");
        wait(cx, 3);
        assert_eq!(candidates(cx), ["app", "lib", "docs"]);
        assert_eq!(
            editor.update(cx, |e, _| e.core.frame().minibuffer.unwrap().prompt),
            "Grep root: "
        );
        cx.simulate_input("dc");
        wait(cx, 1);
        assert_eq!(candidates(cx), ["docs"]);
        cx.simulate_keystrokes("enter");
        cx.simulate_input("needle");
        wait(cx, 1);
        assert_eq!(candidates(cx), ["  1: needle docs"]);
        assert_eq!(
            editor.update(cx, |e, _| e.core.frame().minibuffer.unwrap().groups),
            [(0, "a".into())]
        );
        assert_eq!(
            editor.update(cx, |e, _| e.core.frame().minibuffer.unwrap().prompt),
            "Grep in docs: "
        );
        cx.simulate_keystrokes("enter");
        for _ in 0..1000 {
            cx.run_until_parked();
            if editor.read_with(cx, |e, _| {
                e.core.current().file.as_ref() == Some(&base.join("docs/a"))
            }) {
                break;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        assert_eq!(
            editor.read_with(cx, |e, _| (e.core.current().file.clone(), e.core.point())),
            (Some(base.join("docs/a")), 0)
        );
        cx.update(|window, _| window.remove_window());
        drop(editor);
        cx.run_until_parked();
    }

    #[gpui::test]
    fn grep_status_keeps_the_echo_cursor_with_the_input(cx: &mut TestAppContext) {
        let _native = crate::native::lock();
        let (editor, cx) = editor(cx, "");
        cx.simulate_keystrokes("ctrl-x p g");
        cx.simulate_input("needle");
        cx.run_until_parked();
        let line = |cx: &mut VisualTestContext| {
            editor.update(cx, |e, _| {
                let mb = e.core.frame().minibuffer.unwrap();
                echo_line(&mb, "")
            })
        };
        let before = line(cx);
        let cursor = cx.debug_bounds("minibuffer-cursor").unwrap();
        let now = Instant::now() + Duration::from_secs(1);
        let search = editor.update(cx, |e, cx| {
            let search = e
                .core
                .handle(now, Msg::Tick)
                .into_iter()
                .find_map(|effect| match effect {
                    Effect::Grep(aiditor_core::GrepOp::Search(search)) => Some(search.id),
                    _ => None,
                })
                .unwrap();
            cx.notify();
            search
        });
        cx.run_until_parked();
        let project = editor.read_with(cx, |e, _| e.core.projects()[0].name.clone());
        let input = format!("   0/0 Grep in {project}: needle");
        assert_eq!(before, (input.clone(), input.len()));
        assert_eq!(line(cx), before);
        let check = |cx: &mut VisualTestContext, status| {
            assert_eq!(
                editor.update(cx, |e, _| e.core.frame().minibuffer.unwrap().status),
                Some(status)
            );
            let prompt = cx.debug_bounds("minibuffer-prompt").unwrap();
            let label = cx.debug_bounds("grep-status").unwrap();
            assert_eq!(label.right(), prompt.right());
            assert_eq!(
                cx.debug_bounds("minibuffer-cursor").unwrap().origin.x,
                cursor.origin.x
            );
            assert!(label.left() > cursor.right());
        };
        check(cx, "[searching]");
        editor.update(cx, |e, cx| {
            e.core.handle(
                now,
                Msg::Grepped {
                    search,
                    hits: Ok(Vec::new()),
                    done: true,
                },
            );
            cx.notify();
        });
        cx.run_until_parked();
        assert_eq!(line(cx), before);
        check(cx, "[no matches]");
        cx.simulate_keystrokes("ctrl-b");
        assert_eq!(line(cx), (input.clone(), input.len() - 1));
        cx.simulate_keystrokes("ctrl-g ctrl-x p g");
        let fresh = format!("   0/0 Grep in {project}: ");
        assert_eq!(line(cx), (fresh.clone(), fresh.len()));
    }

    #[gpui::test]
    fn m_x_runs_a_command_by_name(cx: &mut TestAppContext) {
        let _native = crate::native::lock();
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
    fn fuzzy_completion_input_uses_the_themes_match_face(cx: &mut TestAppContext) {
        let _native = crate::native::lock();
        let (editor, cx) = editor(cx, "");
        cx.simulate_keystrokes("alt-x");
        cx.simulate_input("pff");
        for _ in 0..100 {
            let ready = editor.update(cx, |e, _| {
                e.core
                    .frame()
                    .minibuffer
                    .unwrap()
                    .candidates
                    .contains(&"project-find-file".into())
            });
            if ready {
                break;
            }
            cx.executor().advance_clock(Duration::from_millis(20));
            cx.run_until_parked();
        }
        let check = |cx: &mut VisualTestContext, color, bold| {
            editor.update_in(cx, |e, window, _| {
                let mb = e.core.frame().minibuffer.unwrap();
                let i = mb
                    .candidates
                    .iter()
                    .position(|c| c == "project-find-file")
                    .unwrap();
                assert_eq!(mb.selected, Some(i));
                assert_eq!(mb.annotations[i], "C-x p f");
                let font = e.font(window).0;
                let theme = &e.core.settings().theme;
                let runs = completion_runs(mb.candidates[i].len(), &mb.highlights[i], &font, theme);
                assert_eq!(
                    runs.iter().map(|r| r.len).collect::<Vec<_>>(),
                    [1, 7, 1, 4, 1, 3]
                );
                for (i, run) in runs.iter().enumerate() {
                    let matched = i % 2 == 0;
                    assert_eq!(
                        run.color,
                        rgb(if matched { color } else { theme.foreground() }).into()
                    );
                    assert_eq!(
                        run.font.weight,
                        if matched && bold {
                            gpui::FontWeight::BOLD
                        } else {
                            font.weight
                        }
                    );
                }
                let shaped = window.text_system().shape_line(
                    mb.candidates[i].clone().into(),
                    px(e.core.settings().font_size),
                    &runs,
                    None,
                );
                assert_eq!(shaped.text.as_ref(), "project-find-file");
            });
        };
        check(cx, 0x569cd6, true);
        editor.update(cx, |e, cx| {
            e.handle(
                Msg::SettingsLoaded(Ok(Some(
                    "[themes.default.faces]\ncompletion-match = { fg = 0x123456, bold = false }"
                        .into(),
                ))),
                cx,
            );
        });
        cx.run_until_parked();
        check(cx, 0x123456, false);
        cx.simulate_keystrokes("ctrl-g");
    }

    #[gpui::test]
    fn a_completing_prompt_filters_and_picks(cx: &mut TestAppContext) {
        let _native = crate::native::lock();
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
        let _native = crate::native::lock();
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
        let _native = crate::native::lock();
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
        let _native = crate::native::lock();
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
        let _native = crate::native::lock();
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
    fn a_line_number_ends_before_the_text_and_the_toggle_takes_it_away(cx: &mut TestAppContext) {
        let _native = crate::native::lock();
        let (editor, cx) = editor(cx, "a\nb");
        let gutter = |cx: &mut VisualTestContext| {
            editor.read_with(cx, |e, _| {
                let p = &e.painted[0];
                let number = p.rows[0].number.as_ref();
                let right = number.map(|(x, n)| *x + n.width);
                let number = number.map(|(_, n)| n.text.to_string());
                let cursor = p.cursor.unwrap().left() - p.bounds.left();
                (number, right, p.text_x, cursor)
            })
        };
        let (number, right, text_x, cursor) = gutter(cx);
        assert_eq!(number.as_deref(), Some("1"));
        assert!(text_x > px(22.), "10 px, a digit and 12 px");
        assert_eq!(right, Some(text_x - px(12.)), "12 px before the text");
        assert_eq!(cursor, text_x, "the cursor sits on the text");
        cx.simulate_keystrokes("alt-x");
        cx.simulate_input("display-line-numbers-mode");
        cx.simulate_keystrokes("enter");
        cx.run_until_parked();
        let (number, _, text_x, cursor) = gutter(cx);
        assert_eq!((number, text_x, cursor), (None, px(12.), px(12.)));
    }

    #[gpui::test]
    fn the_zoom_keys_lay_the_frame_out_again(cx: &mut TestAppContext) {
        let _native = crate::native::lock();
        let (editor, cx) = editor(cx, "a\nb");
        let laid_out = |cx: &mut VisualTestContext| {
            editor.read_with(cx, |e, _| {
                let p = &e.painted[0];
                (e.reported.2, p.rows[1].top - p.rows[0].top)
            })
        };
        let (cell, line) = laid_out(cx);
        let bar = cx.debug_bounds("workspace-modeline").unwrap().size.height;
        let [up, reset] = if cfg!(target_os = "macos") {
            ["cmd-=", "cmd-0"]
        } else {
            ["ctrl-=", "ctrl-0"]
        };
        cx.simulate_keystrokes(up);
        cx.run_until_parked();
        let (bigger_cell, bigger_line) = laid_out(cx);
        assert!(bigger_cell > cell, "{bigger_cell:?} after {cell:?}");
        assert!(bigger_line > line, "{bigger_line:?} after {line:?}");
        let bigger_bar = cx.debug_bounds("workspace-modeline").unwrap().size.height;
        assert!(bigger_bar > bar, "{bigger_bar:?} after {bar:?}");
        cx.simulate_keystrokes(reset);
        cx.run_until_parked();
        assert_eq!(laid_out(cx), (cell, line));
    }

    #[gpui::test]
    fn windows_reach_the_frame_edges_with_their_text_inset(cx: &mut TestAppContext) {
        let _native = crate::native::lock();
        let (editor, cx) = editor(cx, "a");
        cx.simulate_keystrokes("alt-x");
        cx.simulate_input("display-line-numbers-mode");
        cx.simulate_keystrokes("enter");
        cx.run_until_parked();
        let width = cx.update(|window, _| window.viewport_size().width);
        let (bounds, text_x, top, cursor) = editor.read_with(cx, |e, _| {
            let p = &e.painted[0];
            (p.bounds, p.text_x, p.rows[0].top, p.cursor.unwrap())
        });
        assert_eq!((bounds.left(), bounds.size.width), (px(0.), width));
        assert_eq!(text_x, px(12.));
        assert_eq!(top, px(0.));
        assert_eq!(cursor.left(), px(12.));
        assert_eq!(cursor.top(), bounds.top());
    }

    #[gpui::test]
    fn the_echo_line_is_as_high_as_a_mode_line_and_never_moves_the_windows(
        cx: &mut TestAppContext,
    ) {
        let _native = crate::native::lock();
        let (editor, cx) = editor(cx, "a");
        let (bottom, height) = editor.read_with(cx, |e, _| {
            let settings = e.core.settings();
            (
                e.painted[0].bounds.bottom(),
                px(settings.mode_line_height()),
            )
        });
        let frame = cx.update(|window, _| window.viewport_size().height);
        assert!(
            (frame - bottom - height).abs() < px(1.),
            "{frame:?} less {bottom:?} is one {height:?} echo line"
        );
        let bar = editor.read_with(cx, |e, _| e.painted[0].mode_line_fill[0].0);
        assert_eq!(bar.size.height, height);
        assert_eq!(bar.bottom(), bottom);
    }

    #[gpui::test]
    fn side_by_side_windows_leave_a_pixel_for_the_divider(cx: &mut TestAppContext) {
        let _native = crate::native::lock();
        let (editor, cx) = editor(cx, "");
        cx.simulate_keystrokes("ctrl-x 3");
        cx.run_until_parked();
        let (left, right) = editor.read_with(cx, |e, _| (e.painted[0].bounds, e.painted[1].bounds));
        assert_eq!(right.left() - left.right(), px(1.));
        cx.simulate_keystrokes("ctrl-x 1 ctrl-x 2 ctrl-x 3");
        cx.run_until_parked();
        let (left, right, lower) = editor.read_with(cx, |e, _| {
            let bounds = |i: usize| e.painted[i].bounds;
            (bounds(0), bounds(1), bounds(2))
        });
        assert_eq!(
            right.left() - left.right(),
            px(1.),
            "above the third window"
        );
        assert_eq!(
            left.size.height, right.size.height,
            "down their full height"
        );
        assert_eq!(lower.top(), left.bottom(), "none above the third");
        assert_eq!(lower.size.width, right.right() - left.left());
        cx.simulate_keystrokes("ctrl-x 1 ctrl-x 2");
        cx.run_until_parked();
        let (upper, lower) =
            editor.read_with(cx, |e, _| (e.painted[0].bounds, e.painted[1].bounds));
        assert_eq!(
            lower.top(),
            upper.bottom(),
            "stacked windows have no divider"
        );
    }

    #[gpui::test]
    fn a_session_mode_line_paints_its_title_and_exit_status(cx: &mut TestAppContext) {
        let _native = crate::native::lock();
        let (editor, cx) = editor(cx, "");
        editor.update(cx, |e, cx| {
            let settings = "[agents.fake]\ncommand = 'unused'";
            e.core.handle(
                Instant::now(),
                Msg::SettingsLoaded(Ok(Some(settings.into()))),
            );
            // Keep the spawn inside the fixture, without starting a service.
            for key in parse_keys("M-x agent-start RET fake RET").unwrap() {
                e.core.handle(Instant::now(), Msg::Key(key));
            }
            cx.notify();
        });
        cx.simulate_keystrokes("shift-tab");
        assert_eq!(
            editor.read_with(cx, |e, _| e.core.message().map(str::to_owned)),
            Some("fake is not ready".into())
        );
        editor.update(cx, |e, cx| {
            e.core.handle(
                Instant::now(),
                Msg::AgentStarted {
                    id: aiditor_core::AgentId(0),
                    result: Err("fixture failure".into()),
                },
            );
            cx.notify();
        });
        cx.run_until_parked();
        editor.read_with(cx, |e, _| {
            let painted = &e.painted[0];
            assert_eq!(painted.mode_line.text, "fake");
            assert_eq!(painted.mode_line_right.text, "● exited");
            assert_eq!(
                painted.mode_line_right_x() + painted.mode_line_right.width,
                painted.bounds.right() - px(TEXT_INSET)
            );
        });
    }

    #[gpui::test]
    fn a_file_mode_line_paints_its_declared_language_version(cx: &mut TestAppContext) {
        let _native = crate::native::lock();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.rs");
        std::fs::write(&path, "fn main() {}\n").unwrap();
        std::fs::write(dir.path().join(".tool-versions"), "rust 1.99.0").unwrap();
        let mut core = Core::default();
        core.open(&path, files::read(&path).unwrap());
        let effects = core.handle(Instant::now(), Msg::Tick);
        for effect in effects {
            if let Effect::File(op) = effect {
                core.handle(Instant::now(), files::run(op));
            }
        }
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
        cx.run_until_parked();
        assert!(editor.read_with(cx, |e, _| {
            e.painted[0].mode_line.text.ends_with("│ Rust 1.99.0")
        }));
        cx.update(|window, _| window.remove_window());
        drop(editor);
        cx.run_until_parked();
    }

    #[gpui::test]
    fn a_mode_lines_right_group_ends_at_the_right_inset(cx: &mut TestAppContext) {
        let _native = crate::native::lock();
        let (editor, cx) = editor(cx, "");
        cx.run_until_parked();
        let (x, width, right) = editor.read_with(cx, |e, _| {
            let p = &e.painted[0];
            (
                p.mode_line_right_x(),
                p.mode_line_right.width,
                p.bounds.right(),
            )
        });
        assert!(width > px(0.));
        assert_eq!(x + width, right - px(TEXT_INSET));
    }

    #[gpui::test]
    fn only_an_inactive_mode_line_has_a_rule(cx: &mut TestAppContext) {
        let _native = crate::native::lock();
        let (editor, cx) = editor(cx, "");
        cx.simulate_keystrokes("ctrl-x 2");
        cx.run_until_parked();
        let (active, inactive, theme) = editor.read_with(cx, |e, _| {
            let fill = |i: usize| e.painted[i].mode_line_fill.clone();
            (fill(0), fill(1), e.core.settings().theme.clone())
        });
        let (mode_line, off) = (FaceName::ModeLine, FaceName::ModeLineInactive);
        assert_eq!(
            active.iter().map(|f| f.1).collect::<Vec<_>>(),
            [theme.face(mode_line).bg.unwrap()]
        );
        let colors: Vec<_> = inactive.iter().map(|f| f.1).collect();
        assert_eq!(
            colors,
            [
                theme.face(off).bg.unwrap(),
                theme.face(mode_line).bg.unwrap()
            ]
        );
        let (bar, rule) = (inactive[0].0, inactive[1].0);
        assert_eq!((rule.origin, rule.size.width), (bar.origin, bar.size.width));
        assert_eq!(rule.size.height, px(1.));
    }

    #[gpui::test]
    fn ligatures_are_off_for_all_text(cx: &mut TestAppContext) {
        let _native = crate::native::lock();
        let (editor, cx) = editor(cx, "");
        let calt =
            cx.update(|window, cx| editor.read(cx).font(window).0.features.is_calt_enabled());
        assert_eq!(calt, Some(false));
    }

    #[gpui::test]
    fn the_cursor_is_drawn_over_the_whole_line(cx: &mut TestAppContext) {
        let _native = crate::native::lock();
        let (editor, cx) = editor(cx, "hello world");
        let cursor = |cx: &mut VisualTestContext| {
            let (bounds, rows) = painted(&editor, cx, 0);
            let (x, text_x) = editor.read_with(cx, |e, _| {
                let p = &e.painted[0];
                (p.cursor.unwrap().left(), p.text_x)
            });
            (rows[0].clone(), x - bounds.left() - text_x)
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
