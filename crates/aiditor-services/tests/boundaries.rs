mod support;

use std::fs;

use aiditor_core::Msg;
use aiditor_core::buffer::hash;
use aiditor_core::key::parse_keys;
use aiditor_services::projects;
use support::Driver;

fn kept_directories(d: &Driver) -> Vec<std::path::PathBuf> {
    d.watched
        .directories
        .iter()
        .flat_map(|dirs| dirs.iter().cloned())
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect()
}

#[test]
fn production_visit_save_and_index_replies_can_be_held() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("repo");
    fs::create_dir(&root).unwrap();
    fs::write(root.join("a.txt"), "original").unwrap();
    let loaded = projects::write(
        &dir.path().join("p.toml"),
        None,
        format!("name = 'p'\n[primary]\npath = {root:?}\n"),
    )
    .unwrap();
    let mut d = Driver::default();
    d.send(Msg::ProjectsLoaded {
        dir: None,
        projects: vec![Ok(loaded)],
    });
    d.core.launch(&root, &root);
    d.send(Msg::Tick);
    assert!(
        d.replies
            .iter()
            .any(|msg| matches!(msg, Msg::Indexed { .. }))
    );
    d.settle();
    assert_eq!(d.watched.roots, vec![(root.clone(), vec![])]);

    d.keys(&format!("C-x C-f C-a C-k {}", root.join("a.txt").display()));
    d.send(Msg::Key(parse_keys("M-RET").unwrap()[0]));
    assert!(
        d.replies
            .iter()
            .any(|msg| matches!(msg, Msg::Opened { .. }))
    );
    d.settle();
    assert_eq!(d.core.text(), "original");
    let origin = d.core.workspace();
    d.keys("saved SPC C-x");
    d.send(Msg::Key(parse_keys("C-s").unwrap()[0]));
    assert_eq!(
        fs::read_to_string(root.join("a.txt")).unwrap(),
        "saved original\n"
    );
    assert!(d.core.current().modified());
    assert!(d.replies.iter().any(|msg| matches!(msg, Msg::Saved { .. })));
    let second = d
        .core
        .new_workspace("second", d.core.project_id("p").unwrap());
    d.settle();
    assert_eq!(d.core.workspace(), second);
    assert_eq!(d.core.text(), "");
    d.core.switch_workspace(origin).unwrap();
    assert_eq!(d.core.current().disk, Some(hash(b"saved original\n")));
    assert_eq!(d.core.text(), "saved original\n");
    assert!(!d.core.current().modified());
}

fn project_driver(root: &std::path::Path) -> Driver {
    let mut d = Driver::default();
    let text = format!(
        "# retained comment\nname = 'p'\nwatch = false # choice\n[primary]\npath = {root:?}\n"
    );
    let loaded = projects::write(&root.join("p.toml"), None, text).unwrap();
    d.send(Msg::ProjectsLoaded {
        dir: Some(root.into()),
        projects: vec![Ok(loaded)],
    });
    d.core.launch(root, root);
    d.settle();
    d
}

#[test]
fn project_edits_recreate_deleted_files_keep_comments_and_report_typed_conflicts() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let file = root.join("p.toml");
    let mut d = project_driver(root);
    fs::remove_file(&file).unwrap();
    d.call("project-set-role", &["p", &root.to_string_lossy(), "Main"]);
    let recreated = fs::read_to_string(&file).unwrap();
    assert!(recreated.starts_with("# retained comment\n"));
    assert!(recreated.contains("watch = false # choice"));
    let loaded = projects::read(root).pop().unwrap().unwrap();
    assert_eq!(loaded.project.primary.role, "Main");
    assert_eq!(d.core.projects().last().unwrap().primary.role, "Main");
    fs::write(&file, format!("{recreated}# external\n")).unwrap();
    d.call("project-set-role", &["p", &root.to_string_lossy(), "Other"]);
    assert!(
        d.core
            .message()
            .unwrap()
            .ends_with("changed on disk; reload with C-x C-r")
    );
    assert_eq!(
        fs::read_to_string(&file).unwrap(),
        format!("{recreated}# external\n")
    );
    assert_eq!(d.core.projects().last().unwrap().primary.role, "Main");
    d.hold_files = true;
    d.call("project-create", &["new", &root.to_string_lossy()]);
    let op = d.files.pop_front().unwrap();
    let occupied = root.join("new.toml");
    fs::write(&occupied, "new occupant").unwrap();
    d.send(aiditor_services::files::run(op));
    assert!(
        d.core
            .message()
            .unwrap()
            .ends_with("new.toml already exists")
    );
    assert_eq!(fs::read_to_string(occupied).unwrap(), "new occupant");
    assert_eq!(
        d.core
            .projects()
            .iter()
            .map(|p| p.name.as_str())
            .collect::<Vec<_>>(),
        ["p"]
    );
}

#[test]
fn project_and_buffer_writes_share_the_disk_check() {
    use aiditor_services::files::{WriteExpectation, WriteFailure, write};
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("p.toml");
    let original = format!("name = 'p'\n[primary]\npath = {:?}\n", dir.path());
    fs::write(&file, &original).unwrap();
    let edited = format!("# project edit\n{original}");
    let barrier = std::sync::Barrier::new(2);
    let (project, buffer) = std::thread::scope(|scope| {
        let project = scope.spawn(|| {
            barrier.wait();
            projects::write(&file, Some(&original), edited.clone())
        });
        let buffer = scope.spawn(|| {
            barrier.wait();
            write(
                &file,
                b"buffer edit",
                WriteExpectation::Update(Some(hash(original.as_bytes()))),
            )
        });
        (project.join().unwrap(), buffer.join().unwrap())
    });
    match project {
        Ok(_) => {
            assert!(matches!(buffer, Err(WriteFailure::Conflict)));
            assert_eq!(fs::read_to_string(file).unwrap(), edited);
        }
        Err(err) => {
            assert!(err.ends_with("changed on disk; reload with C-x C-r"));
            buffer.unwrap();
            assert_eq!(fs::read_to_string(file).unwrap(), "buffer edit");
        }
    }
}

#[test]
fn queued_saves_finish_or_cancel_quit_and_workspace_kill_without_losing_edits() {
    for action in ["save-some", "save-buffers-kill-terminal", "workspace-kill"] {
        for outcome in ["saved", "conflict", "edited"] {
            let dir = tempfile::tempdir().unwrap();
            let mut d = project_driver(dir.path());
            let project = d.core.project_id("p").unwrap();
            let origin = d.core.workspace();
            d.core.new_workspace("other", project);
            d.core.switch_workspace(origin).unwrap();
            if action == "save-some" {
                let declined = dir.path().join("b.txt");
                fs::write(&declined, "keep\n").unwrap();
                d.core
                    .open(&declined, aiditor_services::files::read(&declined).unwrap());
                d.keys("unsaved SPC");
            }
            let file = dir.path().join("a.txt");
            fs::write(&file, "base\n").unwrap();
            d.core
                .open(&file, aiditor_services::files::read(&file).unwrap());
            d.keys("mine SPC");
            d.hold_files = true;
            match action {
                "workspace-kill" => d.call(action, &["p", "a.txt", "Yes"]),
                "save-some" => d.call(
                    "save-buffers-kill-terminal",
                    &["a.txt", "Yes", "b.txt", "No", "No"],
                ),
                _ => d.call(action, &["a.txt", "Yes"]),
            }
            let op = d.files.pop_front().expect("queued save");
            assert!(matches!(op, aiditor_core::FileOp::Save { .. }));
            assert_eq!(d.core.workspace(), origin);
            assert!(d.core.current().modified());
            assert!(!d.quit);
            match outcome {
                "conflict" => fs::write(&file, "external\n").unwrap(),
                "edited" => {
                    d.keys("later SPC");
                }
                _ => {}
            }
            let reply = aiditor_services::files::run(op);
            d.send(reply);
            d.settle();
            let success = outcome == "saved";
            assert_eq!(d.quit, success && action == "save-buffers-kill-terminal");
            assert_eq!(
                d.core.workspaces().iter().any(|w| w.id == origin),
                !(success && action == "workspace-kill")
            );
            if !success || action == "save-some" {
                assert_eq!(d.core.workspace(), origin);
                assert_eq!(d.core.current().modified(), !success);
                assert_eq!(
                    d.core.text(),
                    if outcome == "edited" {
                        "mine later base\n"
                    } else {
                        "mine base\n"
                    }
                );
            }
            assert_eq!(
                fs::read_to_string(file).unwrap(),
                if outcome == "conflict" {
                    "external\n"
                } else {
                    "mine base\n"
                }
            );
        }
    }
}

#[test]
fn concurrent_workspace_saves_keep_the_losing_buffers_edits() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = project_driver(dir.path());
    let file = dir.path().join("shared.txt");
    fs::write(&file, "base\n").unwrap();
    let first = d.core.workspace();
    d.core
        .open(&file, aiditor_services::files::read(&file).unwrap());
    d.keys("first SPC");
    d.hold_files = true;
    d.call("save-buffer", &[]);
    let second = d
        .core
        .new_workspace("second", d.core.project_id("p").unwrap());
    d.core
        .open(&file, aiditor_services::files::read(&file).unwrap());
    d.keys("second SPC");
    d.call("save-buffer", &[]);
    let barrier = std::sync::Barrier::new(2);
    let replies = std::thread::scope(|scope| {
        let handles: Vec<_> = d
            .files
            .drain(..)
            .map(|op| {
                let barrier = &barrier;
                scope.spawn(move || {
                    barrier.wait();
                    aiditor_services::files::run(op)
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
            .filter(|m| matches!(m, Msg::Saved { result: Ok(_), .. }))
            .count(),
        1
    );
    for reply in replies.into_iter().rev() {
        d.send(reply);
    }
    let disk = fs::read_to_string(&file).unwrap();
    for (workspace, text) in [(first, "first base\n"), (second, "second base\n")] {
        d.core.switch_workspace(workspace).unwrap();
        assert_eq!(d.core.text(), text);
        assert_eq!(d.core.current().modified(), disk != text);
    }
}

#[test]
fn real_inventory_drives_limits_shared_roots_and_ignore_refresh() {
    use aiditor_core::Disk;
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("repo");
    fs::create_dir_all(root.join("notes/empty")).unwrap();
    fs::write(root.join("notes/a"), "a").unwrap();
    fs::write(root.join(".ignore"), "notes/\n").unwrap();
    let definition = |name: &str, limit, watch| {
        projects::write(&dir.path().join(format!("{name}.toml")), None,
            format!("name = '{name}'\nwatch = {watch}\nmax_watched_files = {limit}\nignore = ['.ignore']\n[primary]\npath = {root:?}\n")).unwrap()
    };
    let loaded = definition("p", 10, true);
    let mut d = Driver::default();
    d.send(Msg::ProjectsLoaded {
        dir: None,
        projects: vec![Ok(loaded.clone())],
    });
    d.core.launch(&root, &root);
    d.send(Msg::Tick);
    d.settle();
    assert_eq!(kept_directories(&d), std::slice::from_ref(&root));
    d.send(Msg::FilesChanged {
        paths: vec![(root.join("notes/a"), Disk::File)],
        rescan: false,
    });
    d.settle();
    fs::write(root.join(".ignore"), "").unwrap();
    d.send(Msg::FilesChanged {
        paths: vec![(root.join(".ignore"), Disk::File)],
        rescan: false,
    });
    d.settle();
    assert_eq!(
        kept_directories(&d),
        [root.clone(), root.join("notes"), root.join("notes/empty")]
    );
    fs::write(root.join("notes/b"), "b").unwrap();
    d.send(Msg::FilesChanged {
        paths: vec![(root.join("notes/b"), Disk::File)],
        rescan: false,
    });
    assert!(d.replies.iter().any(|r| matches!(r, Msg::Indexed { roots, .. } if roots[0].1.as_ref().unwrap().contains(&"notes/b".into()))));
    d.settle();
    fs::write(root.join(".ignore"), "notes/\n").unwrap();
    d.send(Msg::FilesChanged {
        paths: vec![(root.join(".ignore"), Disk::File)],
        rescan: false,
    });
    d.settle();
    assert_eq!(kept_directories(&d), std::slice::from_ref(&root));

    let zero = definition("zero", 0, true);
    let off = definition("off", 10, false);
    d.send(Msg::ProjectsLoaded {
        dir: None,
        projects: vec![Ok(loaded), Ok(zero), Ok(off)],
    });
    d.settle();
    let original = d.core.workspace();
    d.core
        .new_workspace("zero", d.core.project_id("zero").unwrap());
    d.send(Msg::Tick);
    d.settle();
    assert_eq!(d.watched.roots, [(root.clone(), vec![".ignore".into()])]);
    d.core.kill_workspace(original).unwrap();
    d.send(Msg::Tick);
    d.settle();
    assert_eq!(d.watched.roots.len(), 0);
    d.core
        .new_workspace("off", d.core.project_id("off").unwrap());
    d.call("project-index", &["off"]);
    d.send(Msg::FilesChanged {
        paths: vec![(root.join(".ignore"), Disk::File)],
        rescan: false,
    });
    let off_id = d.core.project_id("off").unwrap();
    assert!(d.replies.iter().all(|r| match r {
        Msg::Indexed { project, .. } => *project != off_id,
        _ => true,
    }));
    d.settle();
    assert_eq!(kept_directories(&d), Vec::<std::path::PathBuf>::new());
}

#[test]
fn explicit_index_and_reload_reread_global_excludes() {
    let Ok(fixture) = std::env::var("AIDITOR_EXCLUDES_FIXTURE") else {
        let dir = tempfile::tempdir().unwrap();
        let config = dir.path().join("gitconfig");
        fs::write(
            &config,
            format!(
                "[core]\nexcludesFile = {}\n",
                dir.path().join("excludes").display()
            ),
        )
        .unwrap();
        let result = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "explicit_index_and_reload_reread_global_excludes",
                "--nocapture",
            ])
            .env("AIDITOR_EXCLUDES_FIXTURE", dir.path())
            .env("GIT_CONFIG_GLOBAL", config)
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}{}",
            String::from_utf8_lossy(&result.stdout),
            String::from_utf8_lossy(&result.stderr)
        );
        return;
    };
    let fixture = std::path::PathBuf::from(fixture);
    let root = fixture.join("repo");
    fs::create_dir_all(root.join("notes")).unwrap();
    fs::write(root.join("notes/a"), "a").unwrap();
    fs::write(fixture.join("excludes"), "notes/\n").unwrap();
    let loaded = projects::write(
        &fixture.join("p.toml"),
        None,
        format!("name = 'p'\n[primary]\npath = {root:?}\n"),
    )
    .unwrap();
    let mut d = Driver::default();
    d.send(Msg::ProjectsLoaded {
        dir: None,
        projects: vec![Ok(loaded.clone())],
    });
    d.core.launch(&root, &root);
    d.send(Msg::Tick);
    d.settle();
    assert_eq!(kept_directories(&d), std::slice::from_ref(&root));
    fs::write(fixture.join("excludes"), "").unwrap();
    d.call("project-index", &["p"]);
    assert_eq!(kept_directories(&d), [root.clone(), root.join("notes")]);
    fs::write(fixture.join("excludes"), "notes/\n").unwrap();
    d.send(Msg::ProjectsLoaded {
        dir: None,
        projects: vec![Ok(loaded)],
    });
    d.settle();
    assert_eq!(kept_directories(&d), [root]);
}

#[test]
fn preview_service_prepares_text_and_reports_invalid_or_unreadable_files() {
    use aiditor_core::{FileOp, PreviewRequest};
    use aiditor_services::files;
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("preview");
    let preview = || {
        let Msg::Previewed { result, .. } = files::run(FileOp::Preview(PreviewRequest {
            session: 7,
            revision: 9,
            path: path.clone(),
        })) else {
            panic!("not a preview");
        };
        result
    };
    assert_eq!(preview().unwrap(), None);
    for (bytes, text) in [
        ("α\nβ".as_bytes(), "α\nβ"),
        (
            b"\xef\xbb\xbfhello\r\nworld\r\n".as_slice(),
            "hello\nworld\n",
        ),
    ] {
        fs::write(&path, bytes).unwrap();
        assert_eq!(preview().unwrap().unwrap().to_string(), text);
    }
    for (bytes, error) in [
        (b"a\0b".as_slice(), "binary file"),
        (b"\xff".as_slice(), "not valid UTF-8"),
    ] {
        fs::write(&path, bytes).unwrap();
        assert_eq!(
            preview().unwrap_err(),
            format!("{}: {error}", path.display())
        );
    }
    fs::set_permissions(&path, fs::Permissions::from_mode(0o0)).unwrap();
    let refused = preview();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
    assert!(
        refused
            .unwrap_err()
            .starts_with(&format!("{}: ", path.display()))
    );
}

#[test]
fn a_held_preview_worker_leaves_query_and_cancel_responsive() {
    use aiditor_core::FileOp;
    use aiditor_core::command::{ArgSpec, Command, Preview};
    use aiditor_services::files;
    use std::sync::mpsc;
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("a"), "α\r\n".repeat(100_000)).unwrap();
    let mut d = project_driver(dir.path());
    d.core.register(Command {
        name: "preview-test",
        spec: ArgSpec {
            prompts: &["Pick: "],
            completion: Some(|core, _| {
                ["a", "b", "c"]
                    .iter()
                    .map(|p| {
                        core.projects()
                            .last()
                            .unwrap()
                            .primary
                            .path
                            .join(p)
                            .to_string_lossy()
                            .into_owned()
                    })
                    .collect()
            }),
            preview: Some(|_, path| Some(Preview::File(path.into()))),
            ..Default::default()
        },
        handler: |_, _| Ok(()),
    });
    d.hold_files = true;
    d.call("preview-test", &[]);
    d.send(Msg::Tick);
    let op = d.files.pop_front().unwrap();
    assert!(matches!(op, FileOp::Preview(_)));
    let (release, held) = mpsc::channel();
    let (entered, started) = mpsc::channel();
    let worker = std::thread::spawn(move || {
        entered.send(()).unwrap();
        held.recv().unwrap();
        files::run(op)
    });
    started.recv().unwrap();
    d.keys("b");
    assert_eq!(d.core.text(), "b");
    d.keys("C-g typed");
    assert_eq!(d.core.text(), "typed");
    release.send(()).unwrap();
    d.send(worker.join().unwrap());
    assert_eq!(d.core.text(), "typed");
    assert_eq!(d.files.len(), 0);
}
