//! Headless core driven by key scripts and a fake clock, for tests.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use tempfile::TempDir;

use crate::{
    Core, Ctx, Effect, Effects, Msg,
    command::Args,
    display::{FrameDisplay, FrameWindow, WindowDisplay},
    key::{Key, KeyCode, parse_keys},
    timer::TimerId,
};

pub struct Harness {
    pub core: Core,
    pub dir: TempDir,
    /// The fake clock; only `advance` moves it.
    pub now: Instant,
    /// The fake system clipboard.
    pub clipboard: Option<String>,
}

impl Harness {
    /// A core plus a temp dir holding `files` as (relative path, contents).
    pub fn new(files: &[(&str, &str)]) -> Self {
        let dir = tempfile::tempdir().unwrap();
        for (name, contents) in files {
            let path = dir.path().join(name);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, contents).unwrap();
        }
        Harness {
            core: Core::default(),
            dir,
            now: Instant::now(),
            clipboard: None,
        }
    }

    pub fn path(&self, name: &str) -> PathBuf {
        self.dir.path().join(name)
    }

    /// Feeds a `kbd`-style script such as `"hello C-x C-s"`, carrying out effects as the app does.
    /// Plain chars arrive as typed text, the way the platform's text input sends them.
    pub fn keys(&mut self, script: &str) -> &mut Self {
        for key in parse_keys(script).unwrap() {
            let msg = match key.code {
                KeyCode::Char(c) if key == Key::plain(key.code) => Msg::Insert {
                    text: c.into(),
                    replace: None,
                },
                _ => {
                    // As the app does, the clipboard goes in only before a key that wants it.
                    if let Some(text) = self.clipboard.clone()
                        && self.core.wants_clipboard(key)
                    {
                        self.send(Msg::Clipboard(text));
                    }
                    Msg::Key(key)
                }
            };
            self.send(msg);
        }
        self
    }

    /// Sends `msg` to the core, carrying out its effects as the app does.
    pub fn send(&mut self, msg: Msg) -> &mut Self {
        let effects = self.core.handle(self.now, msg);
        self.run(effects);
        self.settle();
        self
    }

    /// Waits for the completion matcher's threads, so tests see settled matches.
    fn settle(&mut self) {
        let mut effects = Vec::new();
        self.core.poll_completion(self.now, &mut effects);
        self.run(effects);
        let core = &mut self.core;
        while let (Some(_), Some(matcher)) = (&core.minibuffer, &mut core.matcher)
            && matcher.tick(50).running
        {}
    }

    /// Calls the command `name` with `args`, carrying out its effects as the app does.
    pub fn call(&mut self, name: &str, args: Args) -> &mut Self {
        let mut ctx = self.ctx();
        ctx.call(name, args);
        let effects = ctx.effects;
        self.run(effects);
        self.settle();
        self
    }

    fn run(&mut self, effects: Effects) {
        for effect in effects {
            match effect {
                // As the file service does, minus its atomic write.
                Effect::Save {
                    buffer,
                    tick,
                    path,
                    bytes,
                    hash: expected,
                } => {
                    let on_disk = std::fs::read(&path).ok().map(|b| hash(&b));
                    let result = match on_disk == expected {
                        false => Err(format!("{} changed on disk; not saved", path.display())),
                        true => std::fs::write(&path, &bytes)
                            .map(|()| hash(&bytes))
                            .map_err(|err| format!("{}: {err}", path.display())),
                    };
                    let msg = Msg::Saved {
                        buffer,
                        tick,
                        result,
                    };
                    self.core.handle(self.now, msg);
                }
                // As `files::list` does; the core can't depend on the services crate.
                Effect::ListDir(dir) => {
                    let result = std::fs::read_dir(&dir)
                        .map(|entries| {
                            let mut names: Vec<String> = (entries.flatten())
                                .map(|e| {
                                    let slash = if e.path().is_dir() { "/" } else { "" };
                                    format!("{}{slash}", e.file_name().to_string_lossy())
                                })
                                .collect();
                            names.sort();
                            names
                        })
                        .map_err(|err| format!("{}: {err}", dir.display()));
                    self.send(Msg::Listed { dir, result });
                }
                Effect::Open(path) => {
                    let result = match std::fs::read(&path) {
                        Ok(bytes) => {
                            let h = hash(&bytes);
                            Ok(Some((bytes, h)))
                        }
                        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
                        Err(err) => Err(format!("{}: {err}", path.display())),
                    };
                    self.send(Msg::Opened { path, result });
                }
                Effect::SetClipboard(text) => self.clipboard = Some(text),
                Effect::LoadSettings => {
                    let text = std::fs::read_to_string(self.path("settings.toml")).ok();
                    self.core.handle(self.now, Msg::SettingsLoaded(Ok(text)));
                    let text = std::fs::read_to_string(self.path("keymap.toml")).ok();
                    self.core.handle(self.now, Msg::KeymapLoaded(Ok(text)));
                }
            }
        }
    }

    /// Visits `name` in the temp dir, reading it as the app does; a missing file is empty.
    pub fn open(&mut self, name: &str) -> &mut Self {
        let path = self.path(name);
        let file = std::fs::read(&path).ok().map(|b| (hash(&b), b));
        self.core.open(&path, file.map(|(h, b)| (b, h)));
        self
    }

    pub fn disk(&self, name: &str) -> String {
        std::fs::read_to_string(self.path(name)).unwrap()
    }
}

/// Stands in for the file service's hash.
fn hash(bytes: &[u8]) -> u64 {
    use std::hash::{DefaultHasher, Hasher};
    let mut h = DefaultHasher::new();
    h.write(bytes);
    h.finish()
}

impl Harness {
    /// Moves the fake clock on and wakes the core, as the app does at a deadline.
    pub fn advance(&mut self, d: Duration) -> &mut Self {
        self.now += d;
        self.core.handle(self.now, Msg::Tick);
        self
    }

    fn ctx(&mut self) -> Ctx<'_> {
        Ctx {
            core: &mut self.core,
            now: self.now,
            effects: Vec::new(),
        }
    }

    pub fn start_timer(&mut self, after: Duration, repeat: Option<Duration>, msg: Msg) -> TimerId {
        self.ctx().start_timer(after, repeat, msg)
    }

    pub fn cancel_timer(&mut self, id: TimerId) {
        self.ctx().cancel_timer(id)
    }

    pub fn text(&self) -> String {
        self.core.text()
    }

    pub fn point(&self) -> usize {
        self.core.point()
    }

    /// The display model for a window `rows` text lines high, plus its mode line.
    pub fn display(&mut self, rows: usize) -> WindowDisplay {
        let height = (rows + 1) as f32 * self.core.settings().line_height();
        self.core.display(height)
    }

    /// The frame display after the frontend reports a windows area `width` by `height` px
    /// with 8 px cells.
    pub fn frame(&mut self, width: f32, height: f32) -> FrameDisplay {
        let cell_width = 8.;
        self.send(Msg::Resize {
            width,
            height,
            cell_width,
        });
        self.core.frame()
    }
}

mod tests {
    use std::ops::Range;

    use super::*;
    use crate::HISTORY_MAX;
    use crate::command::{ArgSpec, Command, Prefix};
    use crate::settings::Settings;
    use crate::window::WindowId;

    const MS: Duration = Duration::from_millis(1);

    fn msg(c: char) -> Msg {
        Msg::Key(Key::plain(KeyCode::Char(c)))
    }

    #[test]
    fn types_into_a_buffer() {
        let mut h = Harness::new(&[("src/a.txt", "on disk")]);
        assert_eq!(
            std::fs::read_to_string(h.path("src/a.txt")).unwrap(),
            "on disk"
        );
        h.keys("hello RET wörld C-x C-s");
        assert_eq!(h.text(), "hello\nwörld");
        assert_eq!(h.point(), 11);
        assert!(h.core.current().modified());
        assert_eq!(h.core.current().name, "*scratch*");
        assert!(h.core.current().file.is_none());
    }

    #[test]
    fn undo_restores_text_and_point() {
        let mut h = Harness::new(&[]);
        h.keys("abc <left> DEL C-/");
        assert_eq!((h.text().as_str(), h.point()), ("abc", 2));
        let mut h = Harness::new(&[]);
        h.keys("ab RET c C-/ C-_ C-x u");
        assert_eq!((h.text().as_str(), h.point()), ("", 0));
        h.keys("C-? C-? C-?");
        assert_eq!((h.text().as_str(), h.point()), ("ab\nc", 4));
        assert_eq!(h.core.message(), None);
        h.keys("C-?");
        assert_eq!(h.core.message(), Some("No further redo information"));
    }

    #[test]
    fn a_new_edit_clears_redo() {
        let mut h = Harness::new(&[]);
        h.keys("ab C-/ x C-?");
        assert_eq!(h.text(), "x");
        assert_eq!(h.core.message(), Some("No further redo information"));
    }

    #[test]
    fn typing_amalgamates_up_to_twenty_chars() {
        let mut h = Harness::new(&[]);
        h.keys("hello C-/");
        assert_eq!(h.text(), "");
        h.keys("abcdefghijklmnopqrstuvwxy C-/");
        assert_eq!(h.text(), "abcdefghijklmnopqrst");
        let mut h = Harness::new(&[]);
        h.keys("ab <left> <right> cd C-/");
        assert_eq!(h.text(), "ab");
    }

    #[test]
    fn nothing_to_undo() {
        let mut h = Harness::new(&[]);
        h.keys("C-/");
        assert_eq!((h.text().as_str(), h.point()), ("", 0));
        assert_eq!(h.core.message(), Some("No further undo information"));
    }

    #[test]
    fn redo_ignores_shift() {
        let mut h = Harness::new(&[]);
        h.keys("a C-/");
        let key = Key {
            ctrl: true,
            shift: true,
            ..Key::plain(KeyCode::Char('?'))
        };
        h.core.handle(h.now, Msg::Key(key));
        assert_eq!(h.text(), "a");
    }

    /// Registers `test-cmd` with `spec`; it shows the args it got on the message line.
    fn test_cmd(h: &mut Harness, spec: ArgSpec) {
        h.core.register(Command {
            name: "test-cmd",
            spec,
            handler: |ctx, args| {
                ctx.core.message = Some(format!("{args:?}"));
                Ok(())
            },
        });
    }

    #[test]
    fn calls_a_command_by_name_with_its_args() {
        let mut h = Harness::new(&[]);
        test_cmd(
            &mut h,
            ArgSpec {
                region: false,
                prompts: &["Text: "],
                ..ArgSpec::default()
            },
        );
        let args = Args {
            prefix: Some(Prefix::Number(3)),
            strings: vec!["hello".into()],
            ..Args::default()
        };
        h.call("test-cmd", args.clone());
        assert_eq!(h.core.message(), Some(format!("{args:?}").as_str()));
        h.core.message = None;
        h.call("test-cmd", Args::default());
        assert_eq!(
            h.core.message(),
            None,
            "the handler waits for the minibuffer"
        );
        assert_eq!(h.core.minibuffer().unwrap().prompt, "Text: ");
        h.keys("C-g");
        h.call("no-such-command", Args::default());
        assert_eq!(h.core.message(), Some("No command named no-such-command"));
    }

    #[test]
    fn region_comes_from_the_caller_or_point_and_mark() {
        let mut h = Harness::new(&[]);
        test_cmd(
            &mut h,
            ArgSpec {
                region: true,
                prompts: &[],
                ..ArgSpec::default()
            },
        );
        h.keys("abcde <left>");
        h.call("test-cmd", Args::default());
        assert_eq!(
            h.core.message(),
            Some("The mark is not set now, so there is no region")
        );
        let given = Args {
            region: Some(0..2),
            ..Args::default()
        };
        h.call("test-cmd", given.clone());
        assert_eq!(h.core.message(), Some(format!("{given:?}").as_str()));
        h.core.window.set_mark(&mut h.core.buffers, 1).unwrap();
        h.call("test-cmd", Args::default());
        assert_eq!(
            h.core.message(),
            Some("Args { prefix: None, region: Some(1..4), strings: [] }")
        );
    }

    #[test]
    fn handler_errors_go_to_the_message_line() {
        let mut h = Harness::new(&[]);
        h.core.register(Command {
            name: "fail",
            spec: ArgSpec::default(),
            handler: |_, _| Err("boom".into()),
        });
        h.call("fail", Args::default());
        assert_eq!(h.core.message(), Some("boom"));
    }

    #[test]
    fn command_errors_show_in_the_frame_until_the_next_key() {
        let mut h = on("");
        h.core.register(Command {
            name: "fail",
            spec: ArgSpec::default(),
            handler: |_, _| Err("boom".into()),
        });
        h.call("fail", Args::default());
        insta::assert_debug_snapshot!(h.frame(800., 600.));
        h.keys("a");
        assert_eq!(h.frame(800., 600.).message, None);
    }

    #[test]
    fn echo_area_clears_on_click_not_on_scroll_or_timers() {
        let mut h = twenty_lines();
        h.keys("C-g");
        h.send(Msg::Scroll {
            window: WindowId(0),
            lines: 2,
        });
        h.start_timer(MS, None, Msg::Tick);
        h.advance(2 * MS);
        assert_eq!(h.core.message(), Some("Quit"));
        h.keys("M-z");
        assert_eq!(h.core.message(), Some("M-z is undefined"));
        h.send(Msg::Click {
            window: WindowId(0),
            line: 0,
            col: 0,
        });
        assert_eq!(h.core.message(), None);
    }

    #[test]
    fn builtins_are_callable_by_name() {
        let mut h = Harness::new(&[("a.txt", ""), ("settings.toml", "font_size = 18")]);
        h.open("a.txt");
        h.keys("ab");
        h.call("undo", Args::default());
        assert_eq!(h.text(), "");
        h.call("undo-redo", Args::default());
        assert_eq!(h.text(), "ab");
        h.call("save-buffer", Args::default());
        assert_eq!(h.disk("a.txt"), "ab\n");
        h.call("reload-settings", Args::default());
        assert_eq!(h.core.settings().font_size, 18.0);
    }

    #[test]
    fn opens_edits_and_saves_a_file() {
        let mut h = Harness::new(&[("a.txt", "hello")]);
        h.open("a.txt");
        assert_eq!(h.core.current().name, "a.txt");
        assert_eq!(h.core.current().file, Some(h.path("a.txt")));
        assert!(!h.core.current().modified());
        assert_eq!(h.point(), 0);
        h.keys("<right> X");
        assert!(h.core.current().modified());
        assert_eq!(h.disk("a.txt"), "hello");
        h.keys("C-x C-s");
        assert_eq!(h.disk("a.txt"), "hXello\n");
        assert_eq!(h.text(), "hXello\n");
        assert_eq!(h.point(), 2);
        assert!(!h.core.current().modified());
        let wrote = format!("Wrote {}", h.path("a.txt").display());
        assert_eq!(h.core.message(), Some(wrote.as_str()));
        h.keys("C-x C-s");
        assert_eq!(h.core.message(), Some("(No changes need to be saved)"));
    }

    #[test]
    fn refuses_to_save_over_a_file_changed_on_disk() {
        let mut h = Harness::new(&[("a.txt", "old\n")]);
        h.open("a.txt");
        std::fs::write(h.path("a.txt"), "theirs\n").unwrap();
        h.keys("x C-x C-s");
        assert_eq!(h.disk("a.txt"), "theirs\n");
        assert!(h.core.current().modified());
        assert!(
            h.core
                .message()
                .unwrap()
                .ends_with("changed on disk; not saved")
        );
        // A file that appeared after it was opened as missing.
        h.open("new.txt");
        std::fs::write(h.path("new.txt"), "theirs").unwrap();
        h.keys("x C-x C-s");
        assert_eq!(h.disk("new.txt"), "theirs");
    }

    #[test]
    fn saves_keep_the_bom_and_crlf() {
        let mut h = Harness::new(&[("a.txt", "\u{feff}a\r\nb\r\n")]);
        h.open("a.txt");
        assert_eq!(h.text(), "a\nb\n");
        assert_eq!(h.display(5).mode_line, "-- a.txt (DOS)  1:0");
        h.keys("x C-x C-s");
        assert_eq!(h.disk("a.txt"), "\u{feff}xa\r\nb\r\n");
        // The saved hash is the next save's base.
        h.keys("y C-x C-s");
        assert_eq!(h.disk("a.txt"), "\u{feff}xya\r\nb\r\n");
    }

    #[test]
    fn edits_during_a_save_stay_unsaved() {
        let mut h = Harness::new(&[("a.txt", "")]);
        h.open("a.txt");
        h.keys("x");
        let mut ctx = h.ctx();
        ctx.call("save-buffer", Args::default());
        let effects = ctx.effects;
        h.keys("y");
        h.run(effects);
        assert_eq!(h.disk("a.txt"), "x\n");
        assert!(h.core.current().modified());
        h.keys("C-x C-s");
        assert_eq!(h.disk("a.txt"), "xy\n");
    }

    #[test]
    fn binary_and_non_utf8_files_open_empty_and_read_only() {
        for (name, bytes) in [("a.bin", &b"a\0b"[..]), ("l1.txt", b"caf\xe9")] {
            let mut h = Harness::new(&[]);
            std::fs::write(h.path(name), bytes).unwrap();
            h.open(name);
            assert_eq!(h.text(), "");
            assert!(h.core.message().unwrap().contains(", opened read-only"));
            assert_eq!(h.display(5).mode_line, format!("%% {name}  1:0"));
            h.keys("x");
            assert_eq!(h.core.message(), Some("Buffer is read-only"));
            h.keys("C-x C-s");
            assert_eq!(std::fs::read(h.path(name)).unwrap(), bytes);
        }
    }

    #[test]
    fn missing_file_opens_empty_and_is_written_on_save() {
        let mut h = Harness::new(&[]);
        h.open("new.txt");
        assert_eq!(h.text(), "");
        assert!(!h.path("new.txt").exists());
        h.keys("hi C-x C-s");
        assert_eq!(h.disk("new.txt"), "hi\n");
    }

    #[test]
    fn failed_save_keeps_the_buffer_modified() {
        let mut h = Harness::new(&[]);
        h.open("no/such/dir/a.txt");
        h.keys("x C-x C-s");
        assert!(h.core.current().modified());
        assert_eq!(h.text(), "x\n");
        assert!(h.core.message().unwrap().contains("No such file"));
    }

    #[test]
    fn c_x_then_another_key_does_nothing() {
        let mut h = Harness::new(&[("a.txt", "")]);
        h.open("a.txt");
        h.keys("C-x a C-x C-x b");
        assert_eq!(h.text(), "b");
        assert_eq!(h.disk("a.txt"), "");
    }

    #[test]
    fn del_deletes_back_and_stops_at_the_start() {
        let mut h = Harness::new(&[]);
        h.keys("abc DEL");
        assert_eq!((h.text().as_str(), h.point()), ("ab", 2));
        h.keys("DEL DEL DEL");
        assert_eq!((h.text().as_str(), h.point()), ("", 0));
        assert_eq!(h.core.message(), Some("Beginning of buffer"));
        assert!(h.core.current().modified());
        h.keys("abcd C-u 2 DEL");
        assert_eq!((h.text().as_str(), h.point()), ("ab", 2));
    }

    #[test]
    fn left_and_right_cross_lines_and_stop_at_the_ends() {
        let mut h = Harness::new(&[]);
        h.keys("ab RET cd <left> <left>");
        assert_eq!(h.point(), 3);
        h.keys("<left>");
        assert_eq!(h.point(), 2);
        h.keys("<right> <right> <right> <right>");
        assert_eq!(h.point(), 5);
        h.keys("<left> <left> <left> <left> <left> <left>");
        assert_eq!(h.point(), 0);
    }

    /// A harness visiting `text`, with point at 0.
    fn on(text: &str) -> Harness {
        let mut h = Harness::new(&[("a.txt", text)]);
        h.open("a.txt");
        h
    }

    fn line_start(h: &Harness, line: usize) -> usize {
        h.core.current().char_idx(line, 0).unwrap()
    }

    /// Twenty lines `0` to `19`, shown 5 at a time.
    fn twenty_lines() -> Harness {
        let text: Vec<String> = (0..20).map(|i| i.to_string()).collect();
        let mut h = on(&text.join("\n"));
        h.display(5);
        h
    }

    #[test]
    fn motion_keys_and_arrows_run_motion_commands() {
        let mut h = on("ab\ncd");
        for (keys, name) in [
            ("C-f", "forward-char"),
            ("<right>", "forward-char"),
            ("C-b", "backward-char"),
            ("<left>", "backward-char"),
            ("C-n", "next-line"),
            ("<down>", "next-line"),
            ("C-p", "previous-line"),
            ("<up>", "previous-line"),
            ("M-f", "forward-word"),
            ("M-b", "backward-word"),
            ("C-a", "move-beginning-of-line"),
            ("C-e", "move-end-of-line"),
            ("M-<", "beginning-of-buffer"),
            ("M-S-<", "beginning-of-buffer"),
            ("M->", "end-of-buffer"),
            ("M-S->", "end-of-buffer"),
            ("C-v", "scroll-up-command"),
            ("M-v", "scroll-down-command"),
            ("C-l", "recenter-top-bottom"),
        ] {
            h.keys(keys);
            assert_eq!(h.core.last_command(), Some(name), "{keys}");
        }
        assert!(!h.core.current().modified());
    }

    #[test]
    fn prefix_argument_counts_char_motion() {
        let mut h = on("abcdef");
        h.keys("C-u 3 C-f");
        assert_eq!(h.point(), 3);
        h.keys("M-< C-u C-f");
        assert_eq!(h.point(), 4);
        h.keys("C-f C-- 2 C-f");
        assert_eq!(h.point(), 3);
        h.keys("C-u 2 C-b");
        assert_eq!(h.point(), 1);
    }

    #[test]
    fn char_motion_crosses_lines_and_reports_the_ends() {
        let mut h = on("ab\ncd");
        h.keys("C-e C-f");
        assert_eq!(h.point(), 3);
        h.keys("C-u 5 C-f");
        assert_eq!((h.point(), h.core.message()), (5, Some("End of buffer")));
        h.keys("C-u 9 C-b");
        assert_eq!(
            (h.point(), h.core.message()),
            (0, Some("Beginning of buffer"))
        );
        h.keys("C-f");
        assert_eq!((h.point(), h.core.message()), (1, None));
    }

    #[test]
    fn line_motion_keeps_the_goal_column() {
        let mut h = on("abcd\nx\nwxyz");
        h.keys("C-u 3 C-f C-n");
        assert_eq!(h.point(), 6, "end of x");
        h.keys("C-n");
        assert_eq!(h.point(), 10, "column 3 of wxyz");
        h.keys("C-p C-p");
        assert_eq!(h.point(), 3);
        h.keys("C-n C-b C-n");
        assert_eq!(h.point(), 7, "C-b reset the goal to column 0");
        h.keys("M-< C-u 3 C-f C-u 2 C-n");
        assert_eq!(h.point(), 10, "a count keeps the goal");
    }

    #[test]
    fn line_motion_stops_at_the_ends() {
        let mut h = on("ab\ncd");
        h.keys("C-f C-u 3 C-n");
        assert_eq!((h.point(), h.core.message()), (5, Some("End of buffer")));
        h.keys("M-< C-f C-p");
        assert_eq!(
            (h.point(), h.core.message()),
            (0, Some("Beginning of buffer"))
        );
        h.keys("C-e <down> <up>");
        assert_eq!(h.point(), 2, "arrows are the line commands");
    }

    #[test]
    fn word_motion() {
        let mut h = on("foo, bar baz");
        h.keys("M-f M-f");
        assert_eq!(h.point(), 8);
        h.keys("M-> C-u 2 M-b");
        assert_eq!(h.point(), 5);
        h.keys("C-u 9 M-f");
        assert_eq!((h.point(), h.core.message()), (12, None));
        h.keys("C-- M-f");
        assert_eq!(h.point(), 9, "negative goes back");
        h.keys("C-u 99999999999 M-b");
        assert_eq!(h.point(), 0);
    }

    #[test]
    fn editing_keys_run_editing_commands() {
        let mut h = on("ab cd");
        for (keys, name) in [
            ("x", "self-insert-command"),
            ("RET", "newline"),
            ("C-j", "electric-newline-and-maybe-indent"),
            ("C-o", "open-line"),
            ("C-d", "delete-char"),
            ("DEL", "delete-backward-char"),
            // Kills record themselves as kill-region, so the next kill joins them.
            ("M-d", "kill-region"),
            ("M-DEL", "kill-region"),
        ] {
            h.keys(keys);
            assert_eq!(h.core.last_command(), Some(name), "{keys}");
        }
    }

    #[test]
    fn typing_repeats_by_the_prefix_argument() {
        let mut h = Harness::new(&[]);
        h.keys("C-u 3 a");
        assert_eq!((h.text().as_str(), h.point()), ("aaa", 3));
        assert_eq!(h.core.last_command(), Some("self-insert-command"));
        h.keys("C-/");
        assert_eq!(h.text(), "", "one undo group");
        h.keys("C-u RET");
        assert_eq!((h.text().as_str(), h.point()), ("\n\n\n\n", 4));
        h.keys("C-u 0 b");
        assert_eq!(h.text(), "\n\n\n\n");
        h.keys("C-- a");
        assert_eq!(h.text(), "\n\n\n\n");
        assert_eq!(h.core.message(), Some("Negative repetition argument -1"));
    }

    #[test]
    fn newline_keys() {
        let mut h = Harness::new(&[]);
        h.keys("a C-u 2 C-j");
        assert_eq!((h.text().as_str(), h.point()), ("a\n\n", 3));
        let mut h = on("ab");
        h.keys("C-f C-u 2 C-o");
        assert_eq!((h.text().as_str(), h.point()), ("a\n\nb", 1));
        h.keys("C-- C-o");
        assert_eq!(h.core.message(), Some("Negative repetition argument -1"));
    }

    #[test]
    fn delete_char_and_its_ends() {
        let mut h = on("abc");
        h.keys("C-d");
        assert_eq!((h.text().as_str(), h.point()), ("bc", 0));
        let mut h = on("abc");
        h.keys("M-> C-- 2 C-d");
        assert_eq!((h.text().as_str(), h.point()), ("a", 1));
        let mut h = on("abc");
        h.keys("C-u 2 C-f C-u 2 C-d");
        assert_eq!(h.text(), "abc");
        assert_eq!(h.core.message(), Some("End of buffer"));
        h.keys("C-- 3 DEL");
        assert_eq!(
            h.text(),
            "abc",
            "negative DEL deletes forward, past the end"
        );
        assert_eq!(h.core.message(), Some("End of buffer"));
    }

    #[test]
    fn kill_word_keys() {
        let mut h = on("foo bar baz");
        h.keys("M-d");
        assert_eq!((h.text().as_str(), h.point()), (" bar baz", 0));
        let mut h = on("foo bar baz");
        h.keys("C-u 2 M-d");
        assert_eq!(h.text(), " baz");
        let mut h = on("foo bar");
        h.keys("M-> M-DEL");
        assert_eq!((h.text().as_str(), h.point()), ("foo ", 4));
        h.keys("C-- M-d");
        assert_eq!(
            (h.text().as_str(), h.point()),
            ("", 0),
            "negative goes back"
        );
        let mut h = on("foo bar");
        h.keys("M-> M-DEL C-y");
        assert_eq!(h.text(), "foo bar", "the killed word yanks back");
    }

    #[test]
    fn line_ends() {
        let mut h = on("ab\ncde");
        h.keys("C-u 4 C-f C-a");
        assert_eq!(h.point(), 3);
        h.keys("C-e");
        assert_eq!(h.point(), 6);
        h.keys("M-< C-u 2 C-e");
        assert_eq!(h.point(), 6);
        h.keys("C-u 0 C-a");
        assert_eq!(h.point(), 0, "0 is one line up");
        h.keys("C-u 9 C-e");
        assert_eq!((h.point(), h.core.message()), (6, None));
    }

    #[test]
    fn buffer_ends() {
        let mut h = on("ab\ncd");
        h.keys("C-f C-f M->");
        assert_eq!((h.point(), h.core.message()), (5, Some("Mark set")));
        h.keys("M-<");
        assert_eq!(h.point(), 0);
        assert_eq!(mark(&h), (Some(5), false));
        assert_eq!(h.core.window.ring(&h.core.buffers), [2]);
        let mut h = on("ab\ncd");
        h.keys("C-f C-f C-SPC M->");
        assert_eq!(
            (h.point(), mark(&h)),
            (5, (Some(2), true)),
            "an active region keeps its mark"
        );
        assert_eq!(h.core.window.ring(&h.core.buffers), []);
        let mut h = on(&"123456789\n".repeat(10));
        h.keys("C-u 5 M-<");
        assert_eq!(h.point(), 60);
        h.keys("C-u 3 M->");
        assert_eq!(h.point(), 80, "after the line holding char 70");
        h.keys("C-u M->");
        assert_eq!(h.point(), 100, "plain C-u is no count");
        assert_eq!(mark(&h), (Some(60), false), "plain C-u pushes no mark");
    }

    #[test]
    fn deletes_remove_an_active_region() {
        for keys in ["C-e C-SPC C-a DEL", "C-a C-SPC C-e DEL", "C-SPC C-e C-d"] {
            let mut h = on("hello\nx");
            h.keys(keys);
            assert_eq!((h.text().as_str(), h.point()), ("\nx", 0), "{keys}");
            assert!(!mark(&h).1, "{keys}");
        }
        let mut h = on("hello");
        h.keys("C-SPC C-f C-f C-g DEL");
        assert_eq!(h.text(), "hllo", "an inactive mark deletes one char");
        let mut h = on("hello");
        h.keys("C-SPC C-f C-f C-f C-u 1 DEL");
        assert_eq!(h.text(), "helo", "a prefix deletes chars, not the region");
    }

    /// The mark and whether it is active.
    fn mark(h: &Harness) -> (Option<usize>, bool) {
        (h.core.window.mark(&h.core.buffers), h.core.window.active())
    }

    #[test]
    fn c_spc_sets_the_mark_and_repeats_toggle_it() {
        let mut h = on("hello");
        h.keys("C-SPC");
        assert_eq!(h.core.message(), Some("Mark set"));
        h.keys("C-f C-f");
        assert_eq!((h.point(), mark(&h)), (2, (Some(0), true)));
        h.keys("C-f C-SPC");
        assert_eq!(mark(&h), (Some(3), true));
        assert_eq!(
            h.core.window.ring(&h.core.buffers),
            [0],
            "the old mark is pushed"
        );
        h.keys("C-SPC");
        assert_eq!(
            (mark(&h), h.core.message()),
            ((Some(3), false), Some("Mark deactivated"))
        );
        h.keys("C-SPC");
        assert_eq!(
            (mark(&h), h.core.message()),
            ((Some(3), true), Some("Mark activated"))
        );
    }

    #[test]
    fn c_u_c_spc_pops_the_mark() {
        let mut h = on("hello");
        h.keys("C-u C-SPC");
        assert_eq!(
            (h.point(), h.core.message()),
            (0, Some("No mark set in this buffer"))
        );
        h.keys("C-f C-SPC C-f C-f C-SPC C-e C-u C-SPC");
        assert_eq!((h.point(), mark(&h)), (3, (Some(1), false)));
        assert_eq!(h.core.window.ring(&h.core.buffers), [3]);
        h.keys("C-SPC");
        assert_eq!(mark(&h), (Some(3), true), "C-SPC after a pop sets the mark");
    }

    #[test]
    fn c_x_c_x_exchanges_point_and_mark() {
        let mut h = on("hello");
        h.keys("C-x C-x");
        assert_eq!(
            (h.point(), h.core.message()),
            (0, Some("No mark set in this buffer"))
        );
        h.keys("C-f C-SPC C-SPC C-f C-f C-f C-x C-x");
        assert_eq!((h.point(), mark(&h)), (1, (Some(4), true)));
    }

    #[test]
    fn editing_deactivates_the_mark() {
        let mut h = on("hello");
        h.keys("C-SPC C-f x");
        assert_eq!(mark(&h), (Some(0), false));
        assert_eq!(h.display(1).lines[0].spans.len(), 1, "no region shown");
        h.keys("C-x C-x C-/");
        assert!(!h.core.window.active(), "undo is a change too");
    }

    #[test]
    fn visiting_a_file_drops_the_mark_and_ring() {
        let mut h = Harness::new(&[("a.txt", "abc")]);
        h.keys("x C-SPC C-SPC C-SPC");
        h.open("a.txt");
        assert_eq!(mark(&h), (None, false));
        assert_eq!(h.core.window.ring(&h.core.buffers), []);
    }

    #[test]
    fn region_shows_until_c_g() {
        let mut h = on("hello");
        h.keys("C-SPC C-f C-f");
        insta::assert_debug_snapshot!(h.display(1).lines);
        h.keys("C-g");
        assert_eq!(
            (mark(&h), h.core.message()),
            ((Some(0), false), Some("Quit"))
        );
        insta::assert_debug_snapshot!(h.display(1).lines);
    }

    #[test]
    fn scrolling_by_screenfuls() {
        let mut h = twenty_lines();
        h.keys("C-v");
        assert_eq!((h.core.scroll(5), h.point()), (3, line_start(&h, 3)));
        h.keys("M-v");
        assert_eq!((h.core.scroll(5), h.point()), (0, line_start(&h, 3)));
        h.keys("C-u 2 C-v");
        assert_eq!(h.core.scroll(5), 2);
        h.keys("C-- C-v");
        assert_eq!(h.core.scroll(5), 0, "lone minus scrolls a screenful back");
        h.keys("M-v");
        assert_eq!(
            (h.core.scroll(5), h.core.message()),
            (0, Some("Beginning of buffer"))
        );
        h.keys("C-u 99 C-v");
        assert_eq!(h.core.scroll(5), 19);
        assert_eq!(h.point(), line_start(&h, 19));
        h.keys("C-v");
        assert_eq!(
            (h.core.scroll(5), h.core.message()),
            (19, Some("End of buffer"))
        );
        h.keys("C-u 2 M-v");
        assert_eq!((h.core.scroll(5), h.point()), (17, line_start(&h, 19)));
        h.keys("C-u 4 M-v");
        assert_eq!(h.point(), line_start(&h, 17), "point moves up into view");
    }

    #[test]
    fn recentering() {
        let mut h = twenty_lines();
        h.keys("C-u 10 C-n");
        assert_eq!(h.core.scroll(5), 6);
        for top in [8, 10, 6, 8] {
            h.keys("C-l");
            assert_eq!(h.core.scroll(5), top);
        }
        h.keys("C-u 1 C-l");
        assert_eq!(h.core.scroll(5), 9);
        h.keys("C-- C-l");
        assert_eq!(h.core.scroll(5), 6, "minus is the bottom row");
        h.keys("C-l");
        assert_eq!(h.core.scroll(5), 10, "a prefixed C-l keeps the cycle");
        h.keys("C-u C-l");
        assert_eq!(h.core.scroll(5), 8, "plain C-u recenters");
        h.keys("M-< C-l");
        assert_eq!(h.core.scroll(5), 0, "not above the start");
    }

    #[test]
    fn scrolling_follows_point_through_keys() {
        let mut h = Harness::new(&[("a.txt", "0\n1\n2\n3\n4\n5")]);
        h.keys("x");
        h.open("a.txt");
        assert_eq!((h.core.current().name.as_str(), h.point()), ("a.txt", 0));
        h.keys("<down> <down> <down> <down>");
        assert_eq!(h.core.scroll(3), 2);
        h.keys("<up> <up>");
        assert_eq!(h.core.scroll(3), 2);
        h.keys("<up>");
        assert_eq!(h.core.scroll(3), 1);
        // Visiting it again shows the same buffer where it was.
        h.open("a.txt");
        assert_eq!((h.point(), h.core.scroll(3)), (2, 1));
    }

    #[test]
    fn c_x_c_r_reloads_settings_and_keeps_them_on_error() {
        let mut h = Harness::new(&[("settings.toml", "font_size = 18")]);
        assert_eq!(h.core.settings().font_size, 14.0);
        h.keys("C-x C-r");
        assert_eq!(h.core.settings().font_size, 18.0);
        assert_eq!(h.core.message(), None);
        std::fs::write(h.path("settings.toml"), "font_size = \"big\"").unwrap();
        h.keys("C-x C-r");
        assert_eq!(h.core.settings().font_size, 18.0);
        assert!(h.core.message().unwrap().starts_with("settings: "));
        h.keys("a");
        assert_eq!(h.core.message(), None);
        assert_eq!(h.text(), "a");
    }

    #[test]
    fn c_x_c_r_reloads_keymaps_and_reverts_removed_bindings() {
        let mut h = Harness::new(&[("keymap.toml", "[global]\n\"C-x u\" = \"undo-redo\"")]);
        h.keys("C-x C-r ab C-/");
        assert_eq!(h.text(), "");
        h.keys("C-x u");
        assert_eq!(h.text(), "ab");
        std::fs::remove_file(h.path("keymap.toml")).unwrap();
        h.keys("C-x C-r C-x u");
        assert_eq!(h.text(), "");
        assert_eq!(h.core.message(), None);
    }

    #[test]
    fn changed_binding_takes_effect_on_reload() {
        let mut h = Harness::new(&[("keymap.toml", "[global]\n\"C-c a\" = \"one\"")]);
        echo_commands(&mut h, "C-c z");
        h.keys("C-x C-r C-c a");
        assert_eq!(h.core.message(), Some("one"));
        std::fs::write(h.path("keymap.toml"), "[global]\n\"C-c a\" = \"two\"").unwrap();
        h.keys("C-x C-r C-c a");
        assert_eq!(h.core.message(), Some("two"));
    }

    #[test]
    fn bad_keymap_entries_are_reported_and_the_rest_loads() {
        let user = "[global]\n\"C-c a\" = \"one\"\n\"C-c b\" = \"no-such-command\"\n\"C-xy\" = \"one\"\n[test-mode]\n\"C-c m\" = \"two\"";
        let mut h = Harness::new(&[("keymap.toml", user)]);
        echo_commands(&mut h, "C-c z");
        h.keys("C-x C-r");
        let msg = h.core.message().unwrap().to_owned();
        assert!(msg.starts_with("keymap: "), "{msg}");
        for needle in ["C-c b", "no-such-command", "C-xy"] {
            assert!(msg.contains(needle), "{needle}: {msg}");
        }
        h.keys("C-c a");
        assert_eq!(h.core.message(), Some("one"));
        buffer(&mut h).minor_modes.push("test-mode");
        h.keys("C-c m");
        assert_eq!(h.core.message(), Some("two"));
    }

    #[test]
    fn invalid_keymap_file_keeps_the_keymaps_in_effect() {
        let mut h = Harness::new(&[("keymap.toml", "[global]\n\"C-c a\" = \"one\"")]);
        echo_commands(&mut h, "C-c z");
        h.keys("C-x C-r");
        std::fs::write(h.path("keymap.toml"), "[global").unwrap();
        h.keys("C-x C-r");
        assert!(h.core.message().unwrap().starts_with("keymap: "));
        h.keys("C-c a");
        assert_eq!(h.core.message(), Some("one"));
        h.core
            .handle(h.now, Msg::KeymapLoaded(Err("permission denied".into())));
        // Appended after the command's message, since no key came between.
        assert_eq!(h.core.message(), Some("one; keymap: permission denied"));
    }

    #[test]
    fn unreadable_settings_file_is_reported() {
        let mut h = Harness::new(&[]);
        h.core
            .handle(h.now, Msg::SettingsLoaded(Err("permission denied".into())));
        assert_eq!(h.core.message(), Some("settings: permission denied"));
        assert_eq!(h.core.settings(), &Settings::default());
    }

    #[test]
    fn one_shot_fires_once_at_its_deadline() {
        let mut h = Harness::new(&[]);
        h.start_timer(100 * MS, None, msg('a'));
        h.advance(99 * MS);
        assert_eq!(h.text(), "");
        h.advance(MS).advance(100 * MS);
        assert_eq!(h.text(), "a");
        assert_eq!(h.core.next_deadline(), None);
    }

    #[test]
    fn repeat_fires_each_interval_until_cancelled() {
        let mut h = Harness::new(&[]);
        let id = h.start_timer(100 * MS, Some(100 * MS), msg('a'));
        h.advance(100 * MS).advance(100 * MS).advance(100 * MS);
        assert_eq!(h.text(), "aaa");
        h.cancel_timer(id);
        h.advance(100 * MS);
        assert_eq!(h.text(), "aaa");
    }

    #[test]
    fn late_wake_up_does_not_catch_up() {
        let mut h = Harness::new(&[]);
        h.start_timer(100 * MS, Some(100 * MS), msg('a'));
        h.advance(350 * MS);
        assert_eq!(h.text(), "a");
    }

    #[test]
    fn cancelled_before_deadline_never_fires() {
        let mut h = Harness::new(&[]);
        let id = h.start_timer(100 * MS, None, msg('a'));
        h.cancel_timer(id);
        h.advance(200 * MS);
        assert_eq!(h.text(), "");
    }

    #[test]
    fn due_timers_fire_on_any_message() {
        let mut h = Harness::new(&[]);
        h.start_timer(100 * MS, None, msg('a'));
        h.now += 100 * MS;
        h.keys("b");
        assert_eq!(h.text(), "ba");
    }

    #[test]
    fn next_deadline_is_the_earliest_timer() {
        let mut h = Harness::new(&[]);
        let t0 = h.now;
        h.start_timer(300 * MS, None, msg('a'));
        h.start_timer(100 * MS, None, msg('b'));
        assert_eq!(h.core.next_deadline(), Some(t0 + 100 * MS));
    }

    /// Registers `one`, `two` and `three`, which each fail with their name so it shows on the
    /// message line, and binds `keys` to `one` in the global map.
    fn echo_commands(h: &mut Harness, keys: &str) {
        let handlers: [(&'static str, crate::command::Handler); 3] = [
            ("one", |_, _| Err("one".into())),
            ("two", |_, _| Err("two".into())),
            ("three", |_, _| Err("three".into())),
        ];
        for (name, handler) in handlers {
            h.core.register(Command {
                name,
                spec: ArgSpec::default(),
                handler,
            });
        }
        h.core
            .keymaps
            .get_mut("global")
            .unwrap()
            .bind(keys, "one")
            .unwrap();
    }

    fn bind_mode(h: &mut Harness, mode: &'static str, keys: &str, command: &'static str) {
        let map = h.core.keymaps.entry(mode.into()).or_default();
        map.bind(keys, command).unwrap();
    }

    fn buffer(h: &mut Harness) -> &mut crate::buffer::Buffer {
        let id = h.core.window.buffer();
        h.core.buffers.get_mut(id).unwrap()
    }

    #[test]
    fn minor_mode_shadows_global_until_removed() {
        let mut h = Harness::new(&[]);
        echo_commands(&mut h, "C-c a");
        bind_mode(&mut h, "test-mode", "C-c a", "two");
        h.keys("C-c a");
        assert_eq!(h.core.message(), Some("one"));
        buffer(&mut h).minor_modes.push("test-mode");
        h.keys("C-c a");
        assert_eq!(h.core.message(), Some("two"));
        buffer(&mut h).minor_modes.clear();
        h.keys("C-c a");
        assert_eq!(h.core.message(), Some("one"));
        assert_eq!(h.text(), "");
    }

    #[test]
    fn major_mode_sits_between_minor_and_global() {
        let mut h = Harness::new(&[]);
        echo_commands(&mut h, "M-z");
        bind_mode(&mut h, "major", "M-z", "two");
        bind_mode(&mut h, "minor", "M-z", "three");
        buffer(&mut h).major_mode = "major";
        buffer(&mut h).minor_modes.push("minor");
        h.keys("M-z");
        assert_eq!(h.core.message(), Some("three"));
        buffer(&mut h).minor_modes.clear();
        h.keys("M-z");
        assert_eq!(h.core.message(), Some("two"));
    }

    #[test]
    fn mode_prefix_keeps_global_keys_under_it() {
        let mut h = Harness::new(&[("a.txt", "")]);
        h.open("a.txt");
        echo_commands(&mut h, "C-c a");
        bind_mode(&mut h, "test-mode", "C-x a", "two");
        buffer(&mut h).minor_modes.push("test-mode");
        h.keys("x C-x C-s");
        assert_eq!(h.disk("a.txt"), "x\n");
        h.keys("C-x a");
        assert_eq!(h.core.message(), Some("two"));
    }

    #[test]
    fn transient_map_wins_once() {
        let mut h = Harness::new(&[]);
        echo_commands(&mut h, "C-c a");
        let mut transient = crate::keymap::Keymap::default();
        transient.bind("C-c a", "two").unwrap();
        h.core.transient = Some(transient);
        h.keys("C-c a");
        assert_eq!(h.core.message(), Some("two"));
        h.keys("C-c a");
        assert_eq!(h.core.message(), Some("one"));
        // C-g cancelling a prefix ends the sequence, so it takes the transient map too.
        let mut transient = crate::keymap::Keymap::default();
        transient.bind("C-c a", "two").unwrap();
        h.core.transient = Some(transient);
        h.keys("C-c C-g C-c a");
        assert_eq!(h.core.message(), Some("one"));
    }

    #[test]
    fn two_level_prefix_runs_once() {
        let mut h = Harness::new(&[]);
        echo_commands(&mut h, "C-x 4 f");
        h.keys("C-x 4");
        assert_eq!(h.core.message(), None);
        h.keys("f");
        assert_eq!(h.core.message(), Some("one"));
        h.keys("f");
        assert_eq!((h.text().as_str(), h.core.message()), ("f", None));
    }

    #[test]
    fn c_x_hint_lists_its_bindings_after_the_delay() {
        let mut h = Harness::new(&[]);
        h.keys("C-x").advance(999 * MS);
        assert_eq!(h.core.hint(), None, "before the delay");
        h.advance(MS);
        insta::assert_debug_snapshot!(h.core.hint());
    }

    #[test]
    fn hint_marks_prefixes_and_follows_layers() {
        let mut h = Harness::new(&[]);
        echo_commands(&mut h, "C-x 4 f");
        bind_mode(&mut h, "test-mode", "C-x u", "two");
        buffer(&mut h).minor_modes.push("test-mode");
        h.keys("C-x").advance(1000 * MS);
        let entries = h.core.hint().unwrap().entries;
        for entry in [("4", "+prefix"), ("u", "two"), ("C-s", "save-buffer")] {
            let entry = (entry.0.to_string(), entry.1.to_string());
            assert!(entries.contains(&entry), "{entry:?}: {entries:?}");
        }
        h.keys("4").advance(1000 * MS);
        let hint = h.core.hint().unwrap();
        assert_eq!(hint.keys, "C-x 4");
        let f = ("f".to_string(), "one".to_string());
        assert!(hint.entries.contains(&f), "{:?}", hint.entries);
    }

    #[test]
    fn the_next_key_hides_the_hint_and_restarts_the_delay() {
        let mut h = Harness::new(&[]);
        h.keys("C-x").advance(1000 * MS);
        assert!(h.core.hint().is_some());
        h.keys("C-g a").advance(1000 * MS);
        assert_eq!(h.core.hint(), None, "C-g hides it and no timer is left");
        h.keys("C-x")
            .advance(600 * MS)
            .keys("C-s")
            .advance(600 * MS);
        assert_eq!(h.core.hint(), None, "a finished sequence cancels the timer");
        assert_eq!(h.core.next_deadline(), None);
    }

    #[test]
    fn undefined_keys_are_reported() {
        let mut h = Harness::new(&[]);
        h.keys("C-x C-q");
        assert_eq!(h.core.message(), Some("C-x C-q is undefined"));
        h.keys("M-z");
        assert_eq!(h.core.message(), Some("M-z is undefined"));
        h.keys("TAB");
        assert_eq!(h.core.message(), Some("TAB is undefined"));
        h.keys("ab");
        assert_eq!((h.text().as_str(), h.core.message()), ("ab", None));
    }

    #[test]
    fn c_g_quits_a_prefix_and_on_its_own() {
        let mut h = Harness::new(&[("a.txt", "")]);
        h.open("a.txt");
        h.keys("x C-x C-g");
        assert_eq!(h.core.message(), Some("Quit"));
        h.keys("C-s");
        assert_eq!(h.core.message(), Some("C-s is undefined"));
        assert_eq!(h.disk("a.txt"), "");
        h.keys("C-g");
        assert_eq!(h.core.message(), Some("Quit"));
    }

    /// Binds `C-c t` to `test-insert`, which inserts `hi` and shows this and last command.
    fn insert_cmd(h: &mut Harness) {
        h.core.register(Command {
            name: "test-insert",
            spec: ArgSpec::default(),
            handler: |ctx, _| {
                let core = &mut *ctx.core;
                core.message = Some(format!(
                    "{:?} {:?}",
                    core.this_command(),
                    core.last_command()
                ));
                let point = core.window.point(&core.buffers);
                let id = core.window.buffer();
                core.buffers
                    .get_mut(id)
                    .unwrap()
                    .insert(point, "hi")
                    .unwrap();
                core.window.set_point(&mut core.buffers, point + 2).unwrap();
                Ok(())
            },
        });
        bind_mode(h, "global", "C-c t", "test-insert");
    }

    #[test]
    fn key_sequences_run_commands_and_track_the_last_one() {
        let mut h = Harness::new(&[]);
        insert_cmd(&mut h);
        h.keys("C-c t");
        assert_eq!(h.text(), "hi");
        assert_eq!(h.core.message(), Some("Some(\"test-insert\") None"));
        h.keys("C-c t");
        assert_eq!(h.text(), "hihi");
        assert_eq!(
            h.core.message(),
            Some("Some(\"test-insert\") Some(\"test-insert\")")
        );
        assert_eq!(h.core.this_command(), None);
        h.keys("C-x");
        assert_eq!(h.core.last_command(), Some("test-insert"));
        h.keys("C-g");
        assert_eq!(h.core.last_command(), Some("keyboard-quit"));
        h.keys("C-c t a");
        assert_eq!(h.core.last_command(), Some("self-insert-command"));
        h.keys("C-c t C-x C-q");
        assert_eq!(h.core.last_command(), None);
    }

    #[test]
    fn nested_calls_leave_command_tracking_alone() {
        let mut h = Harness::new(&[]);
        h.core.register(Command {
            name: "test-nested",
            spec: ArgSpec::default(),
            handler: |ctx, _| {
                ctx.call("undo", Args::default());
                Ok(())
            },
        });
        bind_mode(&mut h, "global", "C-c n", "test-nested");
        h.keys("a C-c n");
        assert_eq!(h.text(), "");
        assert_eq!(h.core.last_command(), Some("test-nested"));
    }

    #[test]
    fn records_the_key_stream_until_taken() {
        let mut h = Harness::new(&[]);
        h.keys("x");
        assert_eq!(h.core.take_recording(), None);
        h.core.start_recording();
        h.keys("a C-x C-g b");
        assert_eq!(
            h.core.take_recording(),
            Some(parse_keys("a C-x C-g b").unwrap())
        );
        h.keys("c");
        assert_eq!(h.core.take_recording(), None);
    }

    /// Feeds `keys` and returns the message line.
    fn prefix_after(h: &mut Harness, keys: &str) -> Option<String> {
        h.keys(keys);
        h.core.message().map(str::to_owned)
    }

    #[test]
    fn prefix_argument_keys_build_the_argument() {
        let mut h = Harness::new(&[]);
        test_cmd(&mut h, ArgSpec::default());
        bind_mode(&mut h, "global", "C-c p", "test-cmd");
        let shows = |prefix: Option<Prefix>| {
            Some(format!(
                "{:?}",
                Args {
                    prefix,
                    ..Args::default()
                }
            ))
        };
        for (keys, prefix) in [
            ("C-u 3", Prefix::Number(3)),
            ("C-u", Prefix::Universal(4)),
            ("C-u C-u", Prefix::Universal(16)),
            ("C--", Prefix::Minus),
            ("C-u 1 2", Prefix::Number(12)),
            ("C-u - 5", Prefix::Number(-5)),
            ("C-u - 0 5", Prefix::Number(-5)),
            ("M-4 2", Prefix::Number(42)),
            ("M-4 M-2", Prefix::Number(42)),
            ("C-- 7", Prefix::Number(-7)),
            ("C-u C--", Prefix::Minus),
            ("C-u 3 C-u", Prefix::Number(3)),
        ] {
            let got = prefix_after(&mut h, &format!("{keys} C-c p"));
            assert_eq!(got, shows(Some(prefix)), "{keys}");
        }
        assert_eq!(Prefix::Universal(16).value(), 16);
        assert_eq!(Prefix::Minus.value(), -1);
        assert_eq!(Prefix::Number(3).value(), 3);
        // Only the next command gets it, and C-g or typing clears it.
        assert_eq!(prefix_after(&mut h, "C-u 3 C-c p C-c p"), shows(None));
        assert_eq!(prefix_after(&mut h, "C-u 3 C-g C-c p"), shows(None));
        assert_eq!(prefix_after(&mut h, "C-u C-x C-g C-c p"), shows(None));
        assert_eq!(prefix_after(&mut h, "C-u 3 a C-c p"), shows(None));
        // After digits, C-u ends the number: the next digit types that many times.
        h.keys("C-u 3 C-u 5");
        assert!(h.text().ends_with("aaa555"), "{}", h.text());
    }

    #[test]
    fn prefix_argument_keys_keep_the_last_command() {
        let mut h = Harness::new(&[]);
        insert_cmd(&mut h);
        h.keys("C-c t C-u 3");
        assert_eq!(h.core.last_command(), Some("test-insert"));
        h.keys("C-c t");
        assert_eq!(h.core.last_command(), Some("test-insert"));
    }

    fn insert(text: &str, replace: Option<Range<usize>>) -> Msg {
        Msg::Insert {
            text: text.into(),
            replace,
        }
    }

    #[test]
    fn cmd_z_undoes_and_z_types() {
        let mut h = Harness::new(&[]);
        h.keys("a z");
        assert_eq!(h.text(), "az");
        h.keys("s-z");
        assert_eq!(h.text(), "");
    }

    #[test]
    fn input_methods_replace_typed_text() {
        let mut h = Harness::new(&[]);
        h.keys("e").send(insert("é", Some(0..1)));
        assert_eq!((h.text().as_str(), h.point()), ("é", 1));
        h.keys("C-/");
        assert_eq!(h.text(), "", "one undo removes it");
        h.send(insert("😀e", None)).send(insert("é", Some(2..3)));
        assert_eq!(h.text(), "😀é");
        #[allow(
            clippy::reversed_empty_ranges,
            reason = "a reversed range is one of the bad inputs"
        )]
        for bad in [(2..9), (3..2)] {
            h.send(insert("x", Some(bad.clone())));
            assert_eq!(h.text(), "😀é", "{bad:?}");
            assert!(h.core.message().unwrap().contains("outside"), "{bad:?}");
        }
    }

    #[test]
    fn inserted_text_acts_as_typed_keys() {
        let mut h = Harness::new(&[]);
        h.send(insert("ab", None));
        h.keys("C-x").send(insert("u", None));
        assert_eq!(h.text(), "", "C-x u undoes");
        h.keys("C-u").send(insert("3a", None));
        assert_eq!(h.text(), "aaa");
    }

    #[test]
    fn wheel_scrolls_keeping_point_visible() {
        let mut h = twenty_lines();
        h.core.scroll(3);
        let scroll = |lines| Msg::Scroll {
            window: WindowId(0),
            lines,
        };
        h.send(scroll(5));
        assert_eq!((h.core.scroll(3), h.point()), (5, line_start(&h, 5)));
        h.send(scroll(-2));
        assert_eq!((h.core.scroll(3), h.point()), (3, line_start(&h, 5)));
        h.send(scroll(99)).send(scroll(1));
        assert_eq!((h.core.scroll(3), h.core.message()), (19, None));
        h.send(scroll(-99));
        assert_eq!((h.core.scroll(3), h.core.message()), (0, None));
    }

    #[test]
    fn click_moves_point_and_drops_mark_and_prefix() {
        let mut h = on("abc\ndef");
        h.keys("C-SPC C-f");
        let click = |line, col| Msg::Click {
            window: WindowId(0),
            line,
            col,
        };
        h.send(click(1, 1));
        assert_eq!((h.point(), mark(&h).1), (5, false));
        h.send(click(0, 9));
        assert_eq!(h.point(), 3, "past the line's end");
        h.send(click(5, 0));
        assert_eq!(h.point(), 7, "past the buffer's end");
        h.keys("C-x").send(click(0, 0)).keys("u");
        assert_eq!(h.text(), "uabc\ndef", "no prefix pending");
        h.keys("C-u").send(click(1, 0)).keys("x");
        assert_eq!(h.text(), "uabc\nxdef", "no prefix argument");
        h.keys("C-/");
        assert_eq!(
            h.text(),
            "uabc\ndef",
            "typing after a click is its own undo group"
        );
    }

    /// A buffer holding `text` with point at `p`.
    fn at(text: &str, p: usize) -> Harness {
        let mut h = on(text);
        h.core.window.set_point(&mut h.core.buffers, p).unwrap();
        h
    }

    fn ring(h: &Harness) -> Vec<&str> {
        h.core.kill_ring.iter().map(String::as_str).collect()
    }

    #[test]
    fn only_kill_and_yank_keys_want_the_clipboard() {
        let mut h = on("");
        let key = |s: &str| parse_keys(s).unwrap()[0];
        assert!(h.core.wants_clipboard(key("C-y")));
        assert!(h.core.wants_clipboard(key("s-c")));
        assert!(!h.core.wants_clipboard(key("C-f")));
        assert!(!h.core.wants_clipboard(key("M-y")));
        bind_mode(&mut h, "global", "C-x y", "yank");
        h.keys("C-x");
        assert!(
            h.core.wants_clipboard(key("y")),
            "a prefix that ends in yank"
        );
        assert!(!h.core.wants_clipboard(key("C-y")));
    }

    #[test]
    fn kills_yank_back_and_the_ring_keeps_sixty() {
        let mut h = at("foo bar", 0);
        h.keys("M-d M-> C-y");
        assert_eq!(h.text(), " barfoo");
        let mut h = on("");
        for _ in 0..61 {
            h.keys("x C-SPC C-b C-w C-f");
        }
        assert_eq!(h.core.kill_ring.len(), 60);
    }

    #[test]
    fn kill_line() {
        let mut h = at("ab\ncd", 1);
        h.keys("C-k");
        assert_eq!(h.text(), "a\ncd");
        let mut h = at("ab\ncd", 2);
        h.keys("C-k");
        assert_eq!(h.text(), "abcd", "at the end of a line the newline goes");
        let mut h = at("ab \ncd", 2);
        h.keys("C-k");
        assert_eq!(
            h.text(),
            "abcd",
            "only blanks left, so the newline goes too"
        );
        let mut h = at("a\nb\nc", 0);
        h.keys("C-u 2 C-k");
        assert_eq!(h.text(), "c");
        let mut h = at("ab\ncd", 4);
        h.keys("C-u 0 C-k");
        assert_eq!((h.text().as_str(), h.point()), ("ab\nd", 3));
        let mut h = at("ab", 2);
        h.keys("C-k");
        assert_eq!(
            (h.text().as_str(), h.core.message()),
            ("ab", Some("End of buffer"))
        );
        let mut h = at("ab\ncd", 3);
        h.keys("C-u 5 C-k");
        assert_eq!(h.text(), "ab\n", "past the last line kills to the end");
    }

    #[test]
    fn kill_region_and_copy() {
        let mut h = at("abcd", 1);
        h.keys("C-SPC C-f C-f C-w");
        assert_eq!(
            (h.text().as_str(), h.point(), h.core.window.active()),
            ("ad", 1, false)
        );
        let mut h = at("ab", 0);
        h.keys("C-SPC C-e M-w");
        assert_eq!(
            (h.core.window.active(), h.clipboard.as_deref()),
            (false, Some("ab"))
        );
        h.keys("C-y");
        assert_eq!(h.text(), "abab");
        let mut h = on("ab");
        h.keys("C-w");
        assert_eq!(
            h.core.message(),
            Some("The mark is not set now, so there is no region")
        );
    }

    #[test]
    fn consecutive_kills_join() {
        let mut h = at("ab\ncd", 0);
        h.keys("C-k C-k");
        assert_eq!(
            (ring(&h), h.clipboard.as_deref()),
            (vec!["ab\n"], Some("ab\n"))
        );
        h.keys("M-> C-y");
        assert_eq!(h.text(), "cdab\n");
        let mut h = at("foo bar", 4);
        h.keys("M-d M-DEL C-y");
        assert_eq!(h.text(), "foo bar", "killing backward prepends");
        let mut h = at("ab\ncd", 0);
        h.keys("C-k C-f C-k");
        assert_eq!(ring(&h), ["cd", "ab"]);
        let mut h = at("ab\ncd\nef", 0);
        h.keys("C-SPC M-> C-p C-a C-k M-w C-k");
        assert_eq!(
            ring(&h),
            ["\n", "cdab\n"],
            "M-w joins a kill but a kill after it doesn't"
        );
    }

    #[test]
    fn yank_and_its_prefixes() {
        let mut h = at("ab", 0);
        h.keys("C-k C-y C-y");
        assert_eq!((h.text().as_str(), h.point()), ("abab", 4));
        let mut h = at("a\nb", 0);
        h.keys("C-k C-d C-k C-u 2 C-y");
        assert_eq!(h.text(), "a");
        let mut h = at("ab", 0);
        h.keys("C-k C-u C-y");
        assert_eq!(
            (h.text().as_str(), h.point(), mark(&h)),
            ("ab", 0, (Some(2), false))
        );
        let mut h = on("");
        h.keys("C-y");
        assert_eq!(h.core.message(), Some("Kill ring is empty"));
    }

    #[test]
    fn m_y_cycles_after_a_yank() {
        let mut h = at("a\nb", 0);
        h.keys("C-k C-d C-k C-y M-y");
        assert_eq!(h.text(), "a");
        h.keys("M-y");
        assert_eq!((h.text().as_str(), h.point()), ("b", 1), "wraps round");
        h.keys("C-f M-y");
        assert_eq!(
            (h.text().as_str(), h.core.message()),
            ("b", Some("Previous command was not a yank"))
        );
        let mut h = at("a\nb", 0);
        h.keys("C-k C-d C-k C-u C-y M-y");
        assert_eq!(
            (h.text().as_str(), h.point(), mark(&h).0),
            ("a", 0, Some(1))
        );
    }

    #[test]
    fn clipboard_text_from_elsewhere_joins_the_ring() {
        let mut h = on("");
        h.clipboard = Some("xyz".into());
        h.keys("C-y");
        assert_eq!(h.text(), "xyz");
        let mut h = at("ab", 0);
        h.clipboard = Some("xyz".into());
        h.keys("C-k");
        assert_eq!(h.clipboard.as_deref(), Some("ab"));
        h.keys("C-y M-y");
        assert_eq!(h.text(), "xyz");
        let mut h = at("ab", 0);
        h.keys("C-k C-y");
        assert_eq!(ring(&h), ["ab"], "the editor's own text isn't pushed again");
        let mut h = at("ab\ncd", 0);
        h.keys("C-k");
        h.clipboard = Some("xyz".into());
        h.keys("C-k");
        assert_eq!(
            ring(&h),
            ["\n", "xyz", "ab"],
            "a pushed clipboard ends the run"
        );
    }

    /// Each window's (id, point), in tree order, and the selected id.
    fn windows(h: &Harness) -> (Vec<(usize, usize)>, usize) {
        let core = &h.core;
        let point = |id| match core.others.get(&id) {
            Some(w) => w.point(&core.buffers),
            None => core.point(),
        };
        let all = core.tree.leaves().into_iter().map(|id| (id.0, point(id)));
        (all.collect(), core.selected.0)
    }

    #[test]
    fn splitting_keeps_point_and_selects_the_old_window() {
        let mut h = on("abcdef");
        h.keys("C-f C-f C-f C-x 2");
        assert_eq!(windows(&h), (vec![(0, 3), (1, 3)], 0));
        assert_eq!(h.core.window.mark(&h.core.buffers), None);
        h.keys("C-SPC C-x 3");
        assert_eq!(
            h.core.others[&crate::WindowId(2)].mark(&h.core.buffers),
            None,
            "no mark"
        );
    }

    #[test]
    fn two_windows_on_one_buffer_keep_separate_points() {
        let mut h = on("abcdef");
        h.keys("C-f C-f C-f C-x 2 M->");
        assert_eq!(windows(&h), (vec![(0, 6), (1, 3)], 0));
        h.keys("C-x o");
        assert_eq!((h.point(), h.core.selected.0), (3, 1));
        h.keys("C-x o");
        assert_eq!(h.point(), 6);
        h.keys("C-b C-b C-b C-b C-b X Y");
        assert_eq!(
            windows(&h),
            (vec![(0, 3), (1, 5)], 0),
            "the other point follows the edit"
        );
    }

    #[test]
    fn other_window_cycles_both_ways() {
        let mut h = on("abc");
        h.keys("C-x 2 C-x 2");
        let order: Vec<_> = windows(&h).0.iter().map(|w| w.0).collect();
        assert_eq!(order, [0, 2, 1]);
        h.keys("C-x o");
        assert_eq!(h.core.selected.0, 2);
        h.keys("C-x o C-x o");
        assert_eq!(h.core.selected.0, 0, "wraps to the first");
        h.keys("C-u - 1 C-x o");
        assert_eq!(h.core.selected.0, 1, "backward to the last");
        h.keys("C-u 4 C-x o");
        assert_eq!(h.core.selected.0, 0);
        h.keys("C-x 1 C-x o");
        assert_eq!(h.core.selected.0, 0, "one window does nothing");
    }

    #[test]
    fn deleting_windows() {
        let mut h = on("abcdef");
        h.keys("C-x 0");
        assert_eq!(
            (windows(&h), h.core.message()),
            (
                (vec![(0, 0)], 0),
                Some("Attempt to delete minibuffer or sole ordinary window")
            )
        );
        h.keys("C-f C-x 2 C-x o C-f C-f");
        h.keys("C-x 0");
        assert_eq!(
            windows(&h),
            (vec![(0, 1)], 0),
            "the upper window with its point"
        );
        h.keys("C-x 2 C-x 2 C-x o C-x 1");
        assert_eq!(windows(&h), (vec![(3, 1)], 3), "only the second is left");
        assert_eq!(h.frame(80., 100.).windows[0].rect.height, 100.);
    }

    #[test]
    fn visiting_a_file_in_a_split() {
        let mut h = Harness::new(&[("a.txt", "abc")]);
        h.keys("x C-x 2");
        h.open("a.txt");
        let names: Vec<_> = (h.frame(80., 100.).windows.iter())
            .map(|w| w.display.mode_line.clone())
            .collect();
        assert_eq!(names, ["-- a.txt  1:0", "** *scratch*  1:1"]);
    }

    #[test]
    fn frame_display_of_a_stacked_split() {
        let mut h = on("one\ntwo");
        h.keys("C-n C-x 2");
        insta::assert_debug_snapshot!(h.frame(800., 600.));
    }

    #[test]
    fn frame_display_of_a_side_by_side_split() {
        let mut h = on("one\ntwo");
        h.keys("C-x 3 C-n");
        insta::assert_debug_snapshot!(h.frame(800., 600.));
    }

    #[test]
    fn clicks_select_and_scrolls_keep_the_selected_window() {
        let mut h = twenty_lines();
        h.keys("C-x 3");
        let right = h.frame(800., 600.).windows[1].id;
        h.send(Msg::Scroll {
            window: right,
            lines: 5,
        });
        assert_eq!(windows(&h), (vec![(0, 0), (1, line_start(&h, 5))], 0));
        h.send(Msg::Click {
            window: right,
            line: 7,
            col: 1,
        });
        assert_eq!(windows(&h), (vec![(0, 0), (1, line_start(&h, 7) + 1)], 1));
        h.keys("C-x 0").send(Msg::Click {
            window: right,
            line: 0,
            col: 0,
        });
        assert_eq!(
            windows(&h),
            (vec![(0, 0)], 0),
            "a deleted window's click is dropped"
        );
    }

    #[test]
    fn resizing_re_lays_out_snapped_to_lines() {
        let mut h = on("one");
        h.keys("C-x 2");
        let line = h.core.settings().line_height();
        // Heights in lines, to two places.
        let heights = |h: &mut Harness, lines: f32| -> Vec<f32> {
            let frame = h.frame(800., line * lines);
            let lines = |w: &FrameWindow| (w.rect.height / line * 100.).round() / 100.;
            frame.windows.iter().map(lines).collect()
        };
        assert_eq!(heights(&mut h, 10.), [5., 5.]);
        assert_eq!(heights(&mut h, 7.2), [4., 3.2]);
    }

    /// Registers `ask`, prompting for `prompts` and showing its args, bound to `C-c p`.
    fn ask(h: &mut Harness, prompts: &'static [&'static str]) {
        test_cmd(
            h,
            ArgSpec {
                prompts,
                ..ArgSpec::default()
            },
        );
        bind_mode(h, "global", "C-c p", "test-cmd");
    }

    fn strings(strings: &[&str]) -> String {
        let strings: Vec<String> = strings.iter().map(|s| s.to_string()).collect();
        format!("{strings:?}")
    }

    #[test]
    fn a_key_script_answers_a_prompt() {
        let mut h = on("text");
        ask(&mut h, &["Text: "]);
        h.keys("C-c p hello");
        assert_eq!(h.core.minibuffer().unwrap().prompt, "Text: ");
        assert_eq!(h.text(), "hello");
        h.keys("RET");
        let got = h.core.message().unwrap().to_string();
        assert!(
            got.ends_with(&format!("strings: {} }}", strings(&["hello"]))),
            "{got}"
        );
        assert!(h.core.minibuffer().is_none());
        assert_eq!(
            (h.text().as_str(), h.core.current().modified()),
            ("text", false)
        );
        assert_eq!(h.core.last_command(), Some("test-cmd"));
    }

    #[test]
    fn prompts_chain_and_keep_the_prefix_argument() {
        let mut h = on("");
        ask(&mut h, &["A: ", "B: "]);
        h.keys("C-u 3 C-c p a RET");
        assert_eq!(h.core.minibuffer().unwrap().prompt, "B: ");
        h.keys("b RET");
        assert_eq!(
            h.core.message(),
            Some(
                format!(
                    "Args {{ prefix: Some(Number(3)), region: None, strings: {} }}",
                    strings(&["a", "b"])
                )
                .as_str()
            )
        );
    }

    #[test]
    fn c_g_aborts_the_minibuffer() {
        let mut h = on("text");
        ask(&mut h, &["Text: "]);
        h.keys("C-c p abc C-g");
        assert!(h.core.minibuffer().is_none());
        assert_eq!(
            (h.text().as_str(), h.core.message()),
            ("text", Some("Quit"))
        );
    }

    #[test]
    fn editing_keys_work_in_the_minibuffer() {
        let mut h = on("");
        ask(&mut h, &["Text: "]);
        h.keys("C-c p helo <left> l C-a x DEL C-e");
        assert_eq!(h.text(), "hello");
        h.keys("RET");
        assert!(h.core.message().unwrap().contains(&strings(&["hello"])));
    }

    #[test]
    fn each_prompt_keeps_a_history() {
        let mut h = on("");
        ask(&mut h, &["Text: "]);
        h.keys("C-c p one RET C-c p two RET C-c p RET C-c p two RET C-c p x");
        h.keys("M-p");
        assert_eq!((h.text().as_str(), h.point()), ("two", 3));
        h.keys("M-p");
        assert_eq!(h.text(), "one", "empty and repeated answers are not kept");
        h.keys("M-p");
        assert_eq!(
            h.core.message(),
            Some("Beginning of history; no preceding item")
        );
        assert_eq!(h.text(), "one");
        h.keys("C-u 2 M-n");
        assert_eq!(h.text(), "x", "back to what was typed");
        h.keys("M-n");
        assert_eq!(
            h.core.message(),
            Some("End of history; no default available")
        );
        assert_eq!(h.text(), "x");
        h.keys("C-g");
        ask(&mut h, &["Other: "]);
        h.keys("C-c p M-p");
        assert_eq!(
            h.core.message(),
            Some("Beginning of history; no preceding item")
        );
    }

    #[test]
    fn history_keeps_a_hundred() {
        let mut h = on("");
        ask(&mut h, &["Text: "]);
        for i in 0..=HISTORY_MAX {
            h.keys(&format!("C-c p {i} RET"));
        }
        h.keys("C-c p C-u 100 M-p");
        assert_eq!(h.text(), "1");
        h.keys("M-p");
        assert_eq!(
            h.core.message(),
            Some("Beginning of history; no preceding item")
        );
    }

    #[test]
    fn the_minibuffer_stays_put_while_open() {
        let mut h = on("text");
        ask(&mut h, &["Text: "]);
        h.keys("C-x 2 C-c p");
        for keys in ["C-x 2", "C-x 3", "C-x o", "C-x 0", "C-x 1"] {
            h.keys(keys);
            assert!(h.core.message().is_some(), "{keys}");
            assert_eq!(h.frame(800., 600.).windows.len(), 2, "{keys}");
            assert!(h.core.minibuffer().is_some(), "{keys}");
        }
        // A prompt from the minibuffer is refused.
        h.call("test-cmd", Args::default());
        assert_eq!(
            h.core.message(),
            Some("Command attempted to use minibuffer while in minibuffer")
        );
        h.keys("ab").send(Msg::Click {
            window: WindowId(0),
            line: 0,
            col: 2,
        });
        assert!(h.core.minibuffer().is_none());
        assert_eq!((h.text().as_str(), h.point()), ("text", 2));
        assert_eq!(h.core.message(), None, "the command did not run");
    }

    #[test]
    fn the_frame_shows_the_minibuffer() {
        let mut h = on("text");
        ask(&mut h, &["Text: "]);
        h.keys("C-c p hi");
        let frame = h.frame(800., 600.);
        assert!(frame.windows.iter().all(|w| !w.selected));
        insta::assert_debug_snapshot!(frame.minibuffer);
        h.keys("C-g");
        assert_eq!(h.frame(800., 600.).minibuffer, None);
    }

    /// Registers `ask` completing over `candidates` at the prompt `Pick: `, bound to `C-c p`.
    fn ask_completing(h: &mut Harness, candidates: fn(&Core, &Args) -> Vec<String>) {
        test_cmd(
            h,
            ArgSpec {
                prompts: &["Pick: "],
                completion: Some(candidates),
                ..ArgSpec::default()
            },
        );
        bind_mode(h, "global", "C-c p", "test-cmd");
    }

    fn fruit(_: &Core, _: &Args) -> Vec<String> {
        ["apple", "banana", "cherry"].map(String::from).into()
    }

    fn listed(h: &mut Harness) -> (Vec<String>, Option<usize>) {
        let mb = h.core.minibuffer().unwrap();
        (mb.candidates, mb.selected)
    }

    #[test]
    fn typing_filters_candidates() {
        let mut h = on("");
        ask_completing(&mut h, fruit);
        h.keys("C-c p");
        assert_eq!(listed(&mut h), (fruit(&h.core, &Args::default()), Some(0)));
        h.keys("an");
        assert_eq!(listed(&mut h), (vec!["banana".into()], Some(0)));
        h.keys("DEL DEL ch");
        assert_eq!(listed(&mut h).0, ["cherry"]);
        h.keys("C-g");
        ask(&mut h, &["Text: "]);
        h.keys("C-c p");
        assert_eq!(listed(&mut h), (vec![], None), "a plain prompt has no list");
    }

    #[test]
    fn c_n_and_c_p_move_the_selection() {
        let mut h = on("");
        ask_completing(&mut h, fruit);
        h.keys("C-c p C-n C-n C-n");
        assert_eq!(listed(&mut h).1, Some(2), "stops at the last");
        h.keys("C-p");
        assert_eq!(listed(&mut h).1, Some(1));
        h.keys("C-u 5 C-p");
        assert_eq!(listed(&mut h).1, Some(0), "stops at the first");
        h.keys("C-n a");
        assert_eq!(listed(&mut h).1, Some(0), "typing selects the first match");
        h.keys("zzz C-n");
        assert_eq!(h.core.message(), Some("No match"));
        h.keys("C-g");
        ask(&mut h, &["Text: "]);
        h.keys("C-c p ab C-p C-n");
        assert_eq!(h.text(), "ab", "C-n and C-p keep their global bindings");
    }

    #[test]
    fn tab_completes_and_ret_picks_the_selection() {
        let mut h = on("");
        ask_completing(&mut h, fruit);
        h.keys("C-c p C-n TAB");
        assert_eq!((h.text().as_str(), h.point()), ("banana", 6));
        h.keys("C-g C-c p ch RET");
        assert!(h.core.message().unwrap().contains(&strings(&["cherry"])));
        h.keys("C-c p zzz TAB");
        assert_eq!(h.core.message(), Some("No match"));
        h.keys("RET");
        assert!(h.core.message().unwrap().contains(&strings(&["zzz"])));
    }

    #[test]
    fn the_frame_shows_the_completion_list() {
        let mut h = on("");
        ask_completing(&mut h, |_, _| {
            (1..=15).map(|i| format!("item {i}")).collect()
        });
        h.keys("C-c p C-u 12 C-n");
        let mb = h.frame(800., 600.).minibuffer.unwrap();
        assert_eq!((mb.candidates.len(), mb.selected), (10, Some(9)));
        insta::assert_debug_snapshot!((mb.candidates, mb.selected));
        h.keys("C-u 9 C-p");
        assert_eq!(
            listed(&mut h).1,
            Some(0),
            "scrolls up only as far as needed"
        );
        assert_eq!(listed(&mut h).0[0], "item 4");
    }

    #[test]
    fn annotations_are_listed_and_not_matched() {
        let mut h = on("");
        test_cmd(
            &mut h,
            ArgSpec {
                prompts: &["Pick: "],
                completion: Some(fruit),
                annotation: Some(|_, c| (c != "cherry").then(|| format!("{c}-note"))),
                ..ArgSpec::default()
            },
        );
        bind_mode(&mut h, "global", "C-c p", "test-cmd");
        h.keys("C-c p");
        let mb = h.core.minibuffer().unwrap();
        assert_eq!(mb.annotations, ["apple-note", "banana-note", ""]);
        h.keys("note");
        assert_eq!(listed(&mut h).0, Vec::<String>::new());
    }

    #[test]
    fn m_x_runs_a_command_by_name() {
        let mut h = on("abc");
        h.keys("C-u 2 M-x forward-char RET");
        assert_eq!(h.point(), 2);
        assert_eq!(h.core.last_command(), Some("forward-char"));
        h.keys("M-x split-below");
        assert_eq!(
            h.core.selected_candidate().as_deref(),
            Some("split-window-below")
        );
        h.keys("C-g M-x zzz-nothing RET");
        assert!(h.core.message().unwrap().contains("zzz-nothing"));
    }

    #[test]
    fn m_x_shows_each_commands_key() {
        let mut h = on("");
        let note = |h: &mut Harness, name: &str| {
            h.keys(&format!("M-x {name}"));
            let mb = h.core.minibuffer().unwrap();
            let i = mb.candidates.iter().position(|c| c == name).unwrap();
            let note = mb.annotations[i].clone();
            h.keys("C-g");
            note
        };
        assert_eq!(note(&mut h, "split-window-below"), "C-x 2");
        assert_eq!(
            note(&mut h, "undo"),
            "C-/",
            "shortest, then first in kbd order"
        );
        assert_eq!(note(&mut h, "exit-minibuffer"), "", "no key runs it here");
        // A key a minor mode binds to something else no longer runs the command.
        bind_mode(&mut h, "m", "C-x 2", "undo");
        buffer(&mut h).minor_modes.push("m");
        assert_eq!(note(&mut h, "split-window-below"), "");
    }

    fn name(h: &Harness) -> &str {
        h.core.current().name.as_str()
    }

    #[test]
    fn c_x_b_lists_buffers_most_recent_first_and_goes_back() {
        let mut h = Harness::new(&[("a", "1"), ("b", "2")]);
        h.open("a").open("b");
        h.core.buffers.create(" hidden");
        h.keys("C-x b");
        assert_eq!(
            listed(&mut h),
            (vec!["a".into(), "*scratch*".into(), "b".into()], Some(0))
        );
        h.keys("RET");
        assert_eq!(name(&h), "a");
        h.keys("C-x b RET");
        assert_eq!(name(&h), "b");
        h.keys("C-x b notes RET");
        assert_eq!((name(&h), h.text().as_str()), ("notes", ""));
    }

    #[test]
    fn two_files_with_one_name_get_unique_buffer_names() {
        let mut h = Harness::new(&[("a/x", ""), ("b/x", "")]);
        h.open("a/x").open("b/x");
        assert_eq!(h.core.buffer_names(true), ["x<2>", "x", "*scratch*"]);
    }

    #[test]
    fn switching_back_restores_point() {
        let mut h = Harness::new(&[("a", "hello")]);
        h.open("a");
        h.keys("C-f C-f C-f C-x b RET");
        assert_eq!((name(&h), h.point()), ("*scratch*", 0));
        h.keys("C-x b a RET");
        assert_eq!((name(&h), h.point()), ("a", 3));
    }

    #[test]
    fn c_x_4_b_switches_the_next_window() {
        let mut h = Harness::new(&[("a", "1")]);
        h.open("a");
        h.keys("C-x 4 b RET");
        let leaves = h.core.tree.leaves();
        assert_eq!((leaves.len(), h.core.selected), (2, leaves[1]));
        assert_eq!(name(&h), "*scratch*");
        let upper = h.core.others[&leaves[0]].buffer();
        assert_eq!(h.core.buffers.get(upper).unwrap().name, "a");
        // With two windows it uses the next one rather than splitting again.
        h.keys("C-x 4 b RET");
        assert_eq!(
            (h.core.tree.leaves().len(), h.core.selected),
            (2, leaves[0])
        );
    }

    fn messages(h: &Harness) -> Option<String> {
        let id = h.core.buffers.find("*Messages*")?;
        Some(h.core.buffers.get(id).unwrap().text().to_string())
    }

    #[test]
    fn echo_messages_and_log_lines_go_to_messages() {
        let mut h = Harness::new(&[]);
        assert_eq!(messages(&h), None);
        h.keys("C-g C-g M-z");
        h.send(Msg::Log("WARN aiditor: careful".into()));
        assert_eq!(
            messages(&h).unwrap(),
            "Quit\nQuit\nM-z is undefined\nWARN aiditor: careful\n"
        );
        // Each part of a joined message gets its own line.
        h.send(Msg::SettingsLoaded(Err("e1".into())));
        h.send(Msg::KeymapLoaded(Err("e2".into())));
        assert!(
            messages(&h)
                .unwrap()
                .ends_with("careful\nsettings: e1\nkeymap: e2\n")
        );
    }

    #[test]
    fn messages_is_read_only_and_made_again_after_a_kill() {
        let mut h = Harness::new(&[]);
        h.keys("C-g C-x b *Messages* RET a");
        assert_eq!(name(&h), "*Messages*");
        assert_eq!(h.core.message(), Some("Buffer is read-only"));
        assert_eq!(h.text(), "Quit\nBuffer is read-only\n");
        h.keys("C-x k RET C-g");
        assert_eq!(messages(&h).unwrap(), "Quit\n");
    }

    #[test]
    fn c_x_k_kills_the_current_buffer_in_every_window() {
        let mut h = Harness::new(&[("a", "1")]);
        h.open("a");
        h.keys("C-x 2 C-x k");
        assert_eq!(listed(&mut h).0, ["a", "*scratch*"]);
        h.keys("RET");
        assert_eq!((name(&h), h.core.buffers.find("a")), ("*scratch*", None));
        let other = h.core.others[&WindowId(1)].buffer();
        assert_eq!(h.core.buffers.get(other).unwrap().name, "*scratch*");
        h.keys("C-x k nope RET");
        assert_eq!(h.core.message(), Some("No such buffer nope"));
        // That message made *Messages*.
        assert_eq!(h.core.buffers.recent().count(), 2);
        h.keys("C-x k *Messages* RET");
        assert_eq!(h.core.buffers.recent().count(), 1);
        // Killing the last buffer leaves a new, empty *scratch*.
        h.keys("x C-x k RET");
        assert_eq!((name(&h), h.text().as_str()), ("*scratch*", ""));
        assert_eq!(h.core.buffers.recent().count(), 1);
    }

    #[test]
    fn killing_a_modified_file_buffer_asks() {
        let mut h = Harness::new(&[("a", "1")]);
        h.open("a");
        h.keys("x C-x k RET");
        assert_eq!(
            h.core.minibuffer().unwrap().prompt,
            "Buffer modified; kill anyway? "
        );
        assert_eq!(listed(&mut h).0, ["no", "yes"]);
        h.keys("RET");
        assert_eq!((name(&h), h.text().as_str()), ("a", "x1"));
        h.keys("C-x k RET yes RET");
        assert_eq!(name(&h), "*scratch*");
        assert_eq!(h.core.buffers.find("a"), None);
    }

    /// A harness visiting `d/a.txt`, with `d` holding `a.txt`, `b.txt` and `sub/c.txt`.
    fn in_dir() -> Harness {
        let mut h = Harness::new(&[("d/a.txt", "a"), ("d/b.txt", "hi"), ("d/sub/c.txt", "")]);
        h.open("d/a.txt");
        h
    }

    /// Types the temp dir's path for `name` as the platform's text input would.
    fn type_path(h: &mut Harness, name: &str) {
        let text = typed(h, name);
        h.send(Msg::Insert {
            text,
            replace: None,
        });
    }

    /// The temp dir's path for `name`, as typed at a prompt.
    fn typed(h: &Harness, name: &str) -> String {
        h.path(name).to_string_lossy().into_owned()
    }

    #[test]
    fn find_file_starts_at_the_buffer_directory_and_lists_it() {
        let mut h = in_dir();
        h.keys("C-x C-f");
        assert_eq!(h.core.minibuffer().unwrap().prompt, "Find file: ");
        assert_eq!(h.text(), typed(&h, "d/"));
        assert_eq!(listed(&mut h).0, ["a.txt", "b.txt", "sub/"]);
        h.keys("b");
        assert_eq!(listed(&mut h).0, ["b.txt", "sub/"]);
        h.keys("DEL sub/");
        assert_eq!(listed(&mut h).0, ["c.txt"]);
        h.keys("C-g");
        // A buffer without a file starts at the working directory.
        h.keys("C-x b *scratch* RET C-x C-f");
        let cwd = std::env::current_dir().unwrap().join("");
        assert_eq!(h.text(), cwd.to_string_lossy());
    }

    #[test]
    fn find_file_visits_an_existing_or_a_new_file() {
        let mut h = in_dir();
        h.keys("C-x C-f b.txt RET");
        assert_eq!((name(&h), h.text().as_str()), ("b.txt", "hi"));
        assert_eq!(h.core.current().file, Some(h.path("d/b.txt")));
        assert!(!h.core.current().modified());
        // M-RET keeps a new name that fuzzy-matches b.txt.
        h.keys("C-x C-f bt M-RET");
        assert_eq!((name(&h), h.text().as_str()), ("bt", ""));
        assert!(!h.path("d/bt").exists());
        h.keys("x C-x C-s");
        assert_eq!(h.disk("d/bt"), "x\n");
    }

    #[test]
    fn tab_completes_and_ret_enters_a_directory() {
        let mut h = in_dir();
        h.keys("C-x C-f b TAB");
        assert_eq!(h.text(), typed(&h, "d/b.txt"));
        h.keys("C-a C-k");
        type_path(&mut h, "d/s");
        h.keys("RET");
        assert_eq!(h.text(), typed(&h, "d/sub/"));
        assert_eq!(listed(&mut h).0, ["c.txt"]);
        h.keys("RET");
        assert_eq!(name(&h), "c.txt");
    }

    #[test]
    fn find_file_switches_to_a_visited_file() {
        let mut h = in_dir();
        h.keys("x C-x b *scratch* RET C-x C-f C-a C-k");
        type_path(&mut h, "d/a.txt");
        h.keys("RET");
        assert_eq!((name(&h), h.text().as_str()), ("a.txt", "xa"));
        assert_eq!(h.core.buffers.recent().count(), 2);
        // A read that raced another find of the same file switches too.
        h.send(Msg::Opened {
            path: h.path("d/a.txt"),
            result: Ok(None),
        });
        assert_eq!(
            (h.core.buffers.recent().count(), h.text().as_str()),
            (2, "xa")
        );
    }

    #[test]
    fn find_file_refuses_binary_files_and_directories() {
        let mut h = in_dir();
        std::fs::write(h.path("d/bin"), b"a\0b").unwrap();
        h.keys("C-x C-f bin M-RET");
        assert_eq!(name(&h), "a.txt");
        let msg = format!("{}: binary file", h.path("d/bin").display());
        assert_eq!(h.core.message(), Some(msg.as_str()));
        h.keys("C-x C-f M-RET");
        assert_eq!(name(&h), "a.txt");
        assert!(h.core.message().unwrap().starts_with(&typed(&h, "d/")));
        // a.txt, *scratch* and *Messages*.
        assert_eq!(h.core.buffers.recent().count(), 3);
    }

    #[test]
    fn a_listing_error_shows_and_a_late_listing_is_dropped() {
        let mut h = in_dir();
        h.keys("C-x C-f nope/");
        assert!(h.core.message().unwrap().starts_with(&typed(&h, "d/nope/")));
        assert_eq!(listed(&mut h).0, Vec::<String>::new());
        h.send(Msg::Listed {
            dir: h.path("d"),
            result: Ok(vec!["stale".into()]),
        });
        assert_eq!(listed(&mut h).0, Vec::<String>::new());
    }

    #[test]
    fn a_relative_name_lists_and_visits_from_the_buffer_directory() {
        let mut h = in_dir();
        h.keys("C-x C-f C-a C-k sub/");
        assert_eq!(listed(&mut h).0, ["c.txt"]);
        h.keys("RET");
        assert_eq!(h.core.current().file, Some(h.path("d/sub/c.txt")));
    }

    #[test]
    fn m_ret_returns_the_text_as_typed() {
        let mut h = on("");
        ask_completing(&mut h, fruit);
        h.keys("C-c p ch M-RET");
        assert!(h.core.message().unwrap().contains(&strings(&["ch"])));
    }

    #[test]
    fn del_deletes_a_whole_directory_in_a_file_prompt() {
        let mut h = in_dir();
        h.keys("C-x C-f sub/");
        h.keys("DEL");
        assert_eq!(h.text(), typed(&h, "d/"));
        h.keys("sub");
        h.keys("DEL");
        assert_eq!(h.text(), typed(&h, "d/"));
        // A file, a name nothing lists, or point before the end: one char.
        h.keys("b.txt DEL");
        assert_eq!(h.text(), typed(&h, "d/b.tx"));
        h.keys("C-a C-k");
        type_path(&mut h, "d/sub/");
        h.keys("<left> DEL");
        assert_eq!(h.text(), typed(&h, "d/su/"));
        // A prefix argument or an active region keeps plain DEL.
        h.keys("C-e b/ C-u 1 DEL");
        assert_eq!(h.text(), typed(&h, "d/su/b"));
        h.keys("/ C-SPC DEL");
        assert_eq!(h.text(), typed(&h, "d/su/b"));
        // Other completing prompts delete a char.
        h.keys("C-g");
        ask_completing(&mut h, fruit);
        h.keys("C-c p ab/ DEL");
        assert_eq!(h.text(), "ab");
    }
}
