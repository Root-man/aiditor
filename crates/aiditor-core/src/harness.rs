//! Headless core driven by key scripts and a fake clock, for tests.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use tempfile::TempDir;

use crate::{Core, Ctx, Effect, Msg, display::WindowDisplay, key::parse_keys, timer::TimerId};

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
            for effect in self.core.handle(self.now, Msg::Key(key)) {
                match effect {
                    Effect::Save { buffer, path, text } => {
                        if std::fs::write(path, text.to_string()).is_ok() {
                            self.core.handle(self.now, Msg::Saved(buffer));
                        }
                    }
                    Effect::LoadSettings => {
                        let text = std::fs::read_to_string(self.path("settings.toml")).ok();
                        self.core.handle(self.now, Msg::SettingsLoaded(Ok(text)));
                    }
                }
            }
        }
        self
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
        assert!(h.core.current().modified());
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

    #[test]
    fn up_and_down_keep_the_column_clamped() {
        let mut h = Harness::new(&[("a.txt", "abcd\nx\nwxyz")]);
        h.open("a.txt");
        h.keys("<up> <right> <right> <right> <down>");
        assert_eq!(h.point(), 6);
        h.keys("<down>");
        assert_eq!(h.point(), 8);
        h.keys("<down>");
        assert_eq!(h.point(), 8);
        h.keys("<up> <up>");
        assert_eq!(h.point(), 1);
        assert!(!h.core.current().modified());
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
}
