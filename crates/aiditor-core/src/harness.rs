//! Headless core driven by key scripts and a fake clock, for tests.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use tempfile::TempDir;

use crate::{
    Core, Ctx, Effect, Effects, Msg, command::Args, display::WindowDisplay, key::parse_keys,
    timer::TimerId,
};

pub struct Harness {
    pub core: Core,
    pub dir: TempDir,
    /// The fake clock; only `advance` moves it.
    pub now: Instant,
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
        }
    }

    pub fn path(&self, name: &str) -> PathBuf {
        self.dir.path().join(name)
    }

    /// Feeds a `kbd`-style script such as `"hello C-x C-s"`, carrying out effects as the app does.
    pub fn keys(&mut self, script: &str) -> &mut Self {
        for key in parse_keys(script).unwrap() {
            let effects = self.core.handle(self.now, Msg::Key(key));
            self.run(effects);
        }
        self
    }

    /// Calls the command `name` with `args`, carrying out its effects as the app does.
    pub fn call(&mut self, name: &str, args: Args) -> &mut Self {
        let mut ctx = self.ctx();
        ctx.call(name, args);
        let effects = ctx.effects;
        self.run(effects);
        self
    }

    fn run(&mut self, effects: Effects) {
        for effect in effects {
            match effect {
                Effect::Save { buffer, path, text } => {
                    if std::fs::write(path, text.to_string()).is_ok() {
                        self.core.handle(self.now, Msg::Saved(buffer));
                    }
                }
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
        let text = std::fs::read_to_string(&path).unwrap_or_default();
        self.core.open(&path, &text);
        self
    }

    pub fn disk(&self, name: &str) -> String {
        std::fs::read_to_string(self.path(name)).unwrap()
    }

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
}

mod tests {
    use super::*;
    use crate::command::{ArgSpec, Command, Prefix};
    use crate::key::{Key, KeyCode};
    use crate::settings::Settings;

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
        assert_eq!(h.core.message(), Some("test-cmd: no answer for \"Text: \""));
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
    fn builtins_are_callable_by_name() {
        let mut h = Harness::new(&[("a.txt", ""), ("settings.toml", "font_size = 18")]);
        h.open("a.txt");
        h.keys("ab");
        h.call("undo", Args::default());
        assert_eq!(h.text(), "");
        h.call("undo-redo", Args::default());
        assert_eq!(h.text(), "ab");
        h.call("save-buffer", Args::default());
        assert_eq!(h.disk("a.txt"), "ab");
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
        assert_eq!(h.disk("a.txt"), "hXello");
        assert!(!h.core.current().modified());
    }

    #[test]
    fn missing_file_opens_empty_and_is_written_on_save() {
        let mut h = Harness::new(&[]);
        h.open("new.txt");
        assert_eq!(h.text(), "");
        assert!(!h.path("new.txt").exists());
        h.keys("hi C-x C-s");
        assert_eq!(h.disk("new.txt"), "hi");
    }

    #[test]
    fn failed_save_keeps_the_buffer_modified() {
        let mut h = Harness::new(&[]);
        h.open("no/such/dir/a.txt");
        h.keys("x C-x C-s");
        assert!(h.core.current().modified());
        assert_eq!(h.text(), "x");
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
            ("M-d", "kill-word"),
            ("M-DEL", "backward-kill-word"),
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
        h.open("a.txt");
        assert_eq!((h.point(), h.core.scroll(3)), (0, 0));
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
        assert_eq!(h.disk("a.txt"), "x");
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
        assert_eq!(hint.entries, [("f".to_string(), "one".to_string())]);
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
}
