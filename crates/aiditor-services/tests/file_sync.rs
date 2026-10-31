//! Native filesystem notifications through the file service and Core, without GPUI.

use std::fs;
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::time::{Duration, Instant};

use aiditor_core::buffer::hash;
use aiditor_core::key::{Key, KeyCode, parse_keys};
use aiditor_core::{Core, Effect, FileOp, Msg};
use aiditor_services::{files, projects, watcher::Watcher};

#[path = "support/native.rs"]
mod native;

const TIMEOUT: Duration = Duration::from_secs(10);

struct Editor {
    core: Core,
    watcher: Watcher,
    reports: Receiver<Msg>,
    root: PathBuf,
    hold_operations: bool,
    operations: Vec<FileOp>,
    hold_replies: bool,
    replies: Vec<Msg>,
    _dir: tempfile::TempDir,
}

impl Editor {
    fn new(contents: &[(&str, &str)], settings: &str) -> Self {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap().join("repo");
        fs::create_dir_all(root.join(".git")).unwrap();
        for (name, text) in contents {
            let path = root.join(name);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, text).unwrap();
        }
        let project = projects::write(
            &dir.path().join("projects/sync.toml"),
            None,
            format!("name = 'sync'\n{settings}\n[primary]\npath = {root:?}\n"),
        )
        .unwrap();
        let (tx, reports) = mpsc::channel();
        let watcher = Watcher::spawn(move |msg| _ = tx.send(msg));
        let mut editor = Self {
            core: Core::default(),
            watcher,
            reports,
            root,
            hold_operations: false,
            operations: Vec::new(),
            hold_replies: false,
            replies: Vec::new(),
            _dir: dir,
        };
        editor.send(Msg::ProjectsLoaded {
            dir: None,
            projects: vec![Ok(project)],
        });
        editor.core.launch(&editor.root, &editor.root);
        editor.open(contents[0].0);
        editor.ready(contents[0].0);
        editor
    }

    fn open(&mut self, name: &str) {
        let path = self.root.join(name);
        self.core.open(&path, files::read(&path).unwrap());
        self.send(Msg::Tick);
    }

    fn keys(&mut self, script: &str) {
        for key in parse_keys(script).unwrap() {
            self.send(match key.code {
                KeyCode::Char(c) if key == Key::plain(key.code) => Msg::Insert {
                    text: c.into(),
                    replace: None,
                },
                _ => Msg::Key(key),
            });
        }
    }

    fn send(&mut self, msg: Msg) {
        let effects = self.core.handle(Instant::now(), msg);
        let mut replies = Vec::new();
        for effect in effects {
            match effect {
                Effect::Terminal(_) | Effect::Agent(_) => {}
                Effect::File(op) if self.hold_operations => self.operations.push(op),
                Effect::File(op) => replies.push(files::run(op)),
                Effect::Watch(set) => self.watcher.set(set),
                Effect::Recovery(_) | Effect::SetClipboard(_) => {}
                Effect::Grep(_)
                | Effect::LoadConfiguration { .. }
                | Effect::CaptureEnvironment { .. }
                | Effect::Quit => {
                    panic!("unexpected effect in file sync test")
                }
            }
        }
        if self.hold_replies {
            self.replies.extend(replies);
        } else {
            for reply in replies {
                self.send(reply);
            }
        }
    }

    fn ready(&mut self, name: &str) {
        let path = self.root.join(name);
        let bytes = fs::read(&path).unwrap();
        let deadline = Instant::now() + Duration::from_secs(30);
        loop {
            assert!(
                Instant::now() < deadline,
                "watcher never ready for {path:?}"
            );
            fs::write(&path, &bytes).unwrap();
            match self.reports.recv_timeout(Duration::from_millis(200)) {
                Ok(msg) => {
                    let ready = matches!(&msg, Msg::FilesChanged { paths, .. }
                        if paths.iter().any(|(p, _)| path.starts_with(p)));
                    self.send(msg);
                    if ready {
                        return;
                    }
                }
                Err(RecvTimeoutError::Timeout) => {}
                Err(err) => panic!("watcher stopped: {err}"),
            }
        }
    }

    fn later_write(&mut self, name: &str) {
        let deadline = Instant::now() + TIMEOUT;
        while let Ok(msg) = self.reports.recv_timeout(Duration::from_millis(200)) {
            assert!(
                Instant::now() < deadline,
                "{} reports never settled",
                std::env::consts::OS
            );
            self.send(msg);
        }
        let path = self.root.join(name);
        fs::write(&path, "later independent write").unwrap();
        loop {
            let msg = self
                .reports
                .recv_timeout(deadline.saturating_duration_since(Instant::now()))
                .unwrap_or_else(|err| {
                    panic!("{} later write to {path:?}: {err}", std::env::consts::OS)
                });
            let observed = matches!(&msg, Msg::FilesChanged { paths, .. } if paths.iter().any(|(p, _)| p == &path));
            self.send(msg);
            if observed {
                return;
            }
        }
    }

    fn wait_for(&mut self, what: &str, done: impl Fn(&Core) -> bool) {
        let deadline = Instant::now() + TIMEOUT;
        while !done(&self.core) {
            assert!(Instant::now() < deadline, "timed out waiting for {what}");
            let remaining = deadline.saturating_duration_since(Instant::now());
            let msg = self.reports.recv_timeout(remaining).unwrap_or_else(|err| {
                panic!("waiting for {what}: {err}; text = {:?}", self.core.text())
            });
            self.send(msg);
        }
    }
}

#[test]
fn atomic_external_saves_revert_every_clean_copy_and_leave_other_buffers_alone() {
    let _native = native::lock();
    let mut editor = Editor::new(&[("a.txt", "old\nkeep\n"), ("other.txt", "other\n")], "");
    assert_eq!(
        editor.core.watch_set().roots,
        vec![(editor.root.clone(), vec![])]
    );
    let first = editor.core.workspace();
    editor.open("other.txt");
    editor.keys("kept SPC C-x C-s");
    let other_tick = editor.core.current().tick();
    editor.open("a.txt");
    editor.keys("C-n");
    let second = editor
        .core
        .new_workspace("second", editor.core.project_id("sync").unwrap());
    editor.open("a.txt");
    editor.keys("C-n");
    editor.ready("a.txt");

    fs::write(editor.root.join("a.tmp"), "atomic\nkeep\n").unwrap();
    fs::rename(editor.root.join("a.tmp"), editor.root.join("a.txt")).unwrap();
    editor.wait_for("atomic save in the second workspace", |c| {
        c.text() == "atomic\nkeep\n"
    });
    assert!(!editor.core.current().modified());
    assert_eq!(editor.core.point(), "atomic\n".len());
    editor.core.switch_workspace(first).unwrap();
    assert_eq!(editor.core.text(), "atomic\nkeep\n");
    assert!(!editor.core.current().modified());
    assert_eq!(editor.core.point(), "atomic\n".len());

    fs::write(editor.root.join("a.txt"), "later\nkeep\n").unwrap();
    editor.wait_for("a write after the atomic replacement", |c| {
        c.text() == "later\nkeep\n"
    });
    assert_eq!(editor.core.current().disk, Some(hash(b"later\nkeep\n")));
    editor.core.switch_workspace(second).unwrap();
    assert_eq!(editor.core.text(), "later\nkeep\n");
    assert!(!editor.core.current().modified());
    editor.keys("C-/");
    assert_eq!(editor.core.text(), "atomic\nkeep\n");
    editor.keys("C-/");
    assert_eq!(editor.core.text(), "old\nkeep\n");

    editor.core.switch_workspace(first).unwrap();
    editor.open("other.txt");
    assert_eq!(editor.core.text(), "kept other\n");
    assert_eq!(editor.core.current().tick(), other_tick);
    assert!(!editor.core.current().modified());
    editor.keys("C-/");
    assert_eq!(editor.core.text(), "other\n");
}

#[test]
fn external_writes_conflict_dirty_copies_while_clean_copies_revert() {
    let _native = native::lock();
    let mut editor = Editor::new(&[("a.txt", "original\n")], "");
    let dirty = editor.core.workspace();
    let base = editor.core.current().disk;
    editor.keys("mine SPC");
    let tick = editor.core.current().tick();
    editor
        .core
        .new_workspace("clean", editor.core.project_id("sync").unwrap());
    editor.open("a.txt");

    fs::write(editor.root.join("a.txt"), "external\n").unwrap();
    editor.wait_for("the clean workspace to revert", |c| {
        c.text() == "external\n"
    });
    assert!(!editor.core.current().modified());
    assert_eq!(editor.core.current().disk, Some(hash(b"external\n")));
    editor.core.switch_workspace(dirty).unwrap();
    assert_eq!(editor.core.text(), "mine original\n");
    assert!(editor.core.current().modified());
    assert_eq!(editor.core.current().disk, base);
    assert_eq!(editor.core.current().tick(), tick);
    assert_eq!(editor.core.current().conflict, Some(hash(b"external\n")));
    assert!(
        editor.core.frame().windows[0]
            .display
            .mode_line
            .text()
            .contains("[conflict]")
    );
    editor.keys("C-x C-s");
    assert_eq!(
        editor.core.frame().minibuffer.unwrap().prompt,
        format!("{} changed on disk: ", editor.root.join("a.txt").display())
    );
    assert_eq!(
        fs::read_to_string(editor.root.join("a.txt")).unwrap(),
        "external\n"
    );
    editor.keys("Take SPC disk RET");
    assert_eq!(editor.core.text(), "external\n");
    assert!(!editor.core.current().modified());
    editor.keys("C-/");
    assert_eq!(editor.core.text(), "mine original\n");
}

#[test]
fn an_own_save_updates_the_other_workspace_and_preserves_its_own_undo() {
    let _native = native::lock();
    let mut editor = Editor::new(&[("a.txt", "original\n")], "");
    let observer = editor.core.workspace();
    let saving = editor
        .core
        .new_workspace("saving", editor.core.project_id("sync").unwrap());
    editor.open("a.txt");
    editor.keys("saved SPC C-x C-s");
    let tick = editor.core.current().tick();
    assert_eq!(
        fs::read_to_string(editor.root.join("a.txt")).unwrap(),
        "saved original\n"
    );
    assert!(!editor.core.current().modified());
    editor.core.switch_workspace(observer).unwrap();
    editor.wait_for("the other workspace to follow the save", |c| {
        c.text() == "saved original\n"
    });
    assert!(!editor.core.current().modified());
    editor.keys("C-/");
    assert_eq!(editor.core.text(), "original\n");

    editor.core.switch_workspace(saving).unwrap();
    assert_eq!(editor.core.current().tick(), tick);
    assert_eq!(editor.core.text(), "saved original\n");
    assert_eq!(editor.core.current().disk, Some(hash(b"saved original\n")));
    editor.keys("C-/");
    assert_eq!(editor.core.text(), "original\n");
}

#[test]
fn visited_files_still_follow_the_disk_without_project_root_watches() {
    let _native = native::lock();
    for settings in ["watch = false", "max_watched_files = 0"] {
        let mut editor = Editor::new(&[("a.txt", "original\n")], settings);
        assert_eq!(editor.core.watch_set().roots, vec![]);
        assert_eq!(
            editor.core.watch_set().files,
            vec![editor.root.join("a.txt")]
        );
        assert!(
            editor.core.frame().windows[0]
                .display
                .mode_line
                .text()
                .contains("[unwatched]")
        );
        fs::write(editor.root.join("a.tmp"), "external\n").unwrap();
        fs::rename(editor.root.join("a.tmp"), editor.root.join("a.txt")).unwrap();
        editor.wait_for(settings, |c| c.text() == "external\n");
        assert!(!editor.core.current().modified());
        assert_eq!(editor.core.current().disk, Some(hash(b"external\n")));
    }
}

#[test]
fn accepted_recovery_covers_lost_directories_ignore_changes_and_replacements() {
    let _native = native::lock();
    let mut editor = Editor::new(
        &[
            ("a", "a"),
            ("kept/old", "old"),
            (".ignore", "hidden/\n.ignore\n"),
            ("hidden/file", "hidden"),
        ],
        "",
    );
    fs::create_dir_all(editor.root.join("lost/deep")).unwrap();
    fs::write(editor.root.join("lost/deep/file"), "lost").unwrap();
    editor.send(Msg::FilesChanged {
        paths: vec![],
        rescan: true,
    });
    assert!(
        editor
            .core
            .watch_set()
            .directories
            .iter()
            .any(|dirs| dirs.contains(&editor.root.join("lost/deep")))
    );
    editor.ready("lost/deep/file");
    editor.later_write("lost/deep/file");

    fs::write(editor.root.join(".ignore"), ".ignore\n").unwrap();
    let hidden = editor.root.join("hidden");
    editor.wait_for("excluded ignore file to restore hidden coverage", |c| {
        c.watch_set()
            .directories
            .iter()
            .any(|dirs| dirs.contains(&hidden))
    });
    editor.ready("hidden/file");
    editor.later_write("hidden/file");

    let kept = editor.root.join("kept");
    fs::rename(&kept, editor._dir.path().join("old-kept")).unwrap();
    fs::create_dir_all(kept.join("deep")).unwrap();
    fs::write(kept.join("deep/new"), "new").unwrap();
    editor.wait_for("replacement directory coverage", |c| {
        c.watch_set()
            .directories
            .iter()
            .any(|dirs| dirs.contains(&kept.join("deep")))
    });
    editor.ready("kept/deep/new");
    editor.later_write("kept/deep/new");
}

#[test]
fn recovery_crossing_the_limit_keeps_visited_file_synchronization() {
    let _native = native::lock();
    let mut editor = Editor::new(&[("a", "original")], "max_watched_files = 2");
    fs::create_dir_all(editor.root.join("new/deep")).unwrap();
    fs::write(editor.root.join("new/deep/b"), "b").unwrap();
    editor.send(Msg::FilesChanged {
        paths: vec![],
        rescan: true,
    });
    assert_eq!(editor.core.watch_set().roots.len(), 0);
    assert_eq!(editor.core.watch_set().files, [editor.root.join("a")]);
    editor.ready("a");
    fs::write(editor.root.join("a"), "fallback after recovery").unwrap();
    editor.wait_for("visited fallback after size-limit recovery", |c| {
        c.text() == "fallback after recovery"
    });
}

#[test]
fn overlapping_workspace_saves_deliver_reversed_replies_before_watch_recovery() {
    let _native = native::lock();
    let mut editor = Editor::new(&[("a.txt", "base\n")], "");
    let first = editor.core.workspace();
    editor.hold_operations = true;
    editor.keys("first SPC C-x C-s");
    let second = editor
        .core
        .new_workspace("second", editor.core.project_id("sync").unwrap());
    editor.open("a.txt");
    editor.keys("second SPC C-x C-s");
    let operations = std::mem::take(&mut editor.operations);
    assert_eq!(operations.len(), 2);
    assert!(
        operations
            .iter()
            .all(|op| matches!(op, FileOp::Save { .. }))
    );
    let barrier = std::sync::Barrier::new(2);
    let replies = std::thread::scope(|scope| {
        let handles: Vec<_> = operations
            .into_iter()
            .map(|op| {
                let barrier = &barrier;
                scope.spawn(move || {
                    barrier.wait();
                    files::run(op)
                })
            })
            .collect();
        handles
            .into_iter()
            .map(|h| h.join().unwrap())
            .collect::<Vec<_>>()
    });
    assert_eq!(
        replies
            .iter()
            .filter(|reply| matches!(reply, Msg::Saved { result: Ok(_), .. }))
            .count(),
        1
    );
    editor.hold_operations = false;
    for reply in replies.into_iter().rev() {
        editor.send(reply);
    }
    let disk = fs::read_to_string(editor.root.join("a.txt")).unwrap();
    for (workspace, expected) in [(first, "first base\n"), (second, "second base\n")] {
        editor.core.switch_workspace(workspace).unwrap();
        assert_eq!(editor.core.text(), expected);
        assert_eq!(editor.core.current().modified(), disk != expected);
    }
    let winner = if disk == "first base\n" {
        first
    } else {
        second
    };
    let loser = if winner == first { second } else { first };
    editor.core.switch_workspace(winner).unwrap();
    fs::write(editor.root.join("a.txt"), "later\n").unwrap();
    editor.wait_for(
        "the successful save's clean buffer to follow a later write",
        |c| c.text() == "later\n",
    );
    editor.core.switch_workspace(loser).unwrap();
    assert_eq!(
        editor.core.text(),
        if loser == first {
            "first base\n"
        } else {
            "second base\n"
        }
    );
    assert!(editor.core.current().modified());
    assert_eq!(editor.core.current().conflict, Some(hash(b"later\n")));
}

#[test]
fn held_reverts_keep_new_edits_and_apply_only_to_their_original_buffer() {
    let _native = native::lock();
    let mut editor = Editor::new(&[("a.txt", "base\n"), ("other.txt", "other\n")], "");
    let first = editor.core.workspace();
    let second = editor
        .core
        .new_workspace("second", editor.core.project_id("sync").unwrap());
    editor.open("a.txt");
    editor.hold_replies = true;
    fs::write(editor.root.join("a.txt"), "disk\n").unwrap();
    editor.send(Msg::FilesChanged {
        paths: vec![],
        rescan: true,
    });
    let checks = std::mem::take(&mut editor.replies);
    for check in checks {
        editor.send(check);
    }
    let replies = std::mem::take(&mut editor.replies);
    assert_eq!(
        replies
            .iter()
            .filter(|reply| matches!(reply, Msg::Reverted { .. }))
            .count(),
        2
    );
    editor.keys("mine SPC");
    editor.core.switch_workspace(first).unwrap();
    editor.open("other.txt");
    editor.hold_replies = false;
    for reply in replies.into_iter().rev() {
        editor.send(reply);
    }
    assert_eq!(editor.core.text(), "other\n");
    editor.keys("C-x b a.txt RET");
    assert_eq!(editor.core.text(), "disk\n");
    editor.core.switch_workspace(second).unwrap();
    assert_eq!(editor.core.text(), "mine base\n");
    assert!(editor.core.current().modified());
}
