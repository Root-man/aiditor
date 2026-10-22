//! Following the disk: what the watcher watches, and what its reports do to buffers and indexes.

use std::path::PathBuf;

use crate::project_commands::FileIndex;
use crate::{Core, Ctx, Disk, Effect, Effects, FileOp, WatchSet};

impl Core {
    /// The roots of every project a workspace is bound to and every file a buffer visits.
    pub fn watch_set(&self) -> WatchSet {
        let home = std::env::home_dir();
        let mut roots: Vec<PathBuf> = (self.projects.iter())
            .filter(|p| self.workspaces.iter().any(|w| w.project == p.project.name))
            // shortcut: the ad-hoc `$HOME` project is never watched, as inotify would need a watch per directory under it; #191 makes it a `watch = false` default.
            .filter(|p| p.file.is_some() || home.as_ref() != Some(&p.project.primary.path))
            .flat_map(|p| p.project.all_roots().map(|r| r.path.clone()))
            .collect();
        roots.sort();
        roots.dedup();
        let mut files: Vec<PathBuf> = (self.buffers.recent())
            .filter_map(|(_, b)| b.file.clone())
            .collect();
        files.sort();
        files.dedup();
        WatchSet { roots, files }
    }

    /// Sends the watch set when it changed since last sent.
    pub(crate) fn watch(&mut self, effects: &mut Effects) {
        let set = self.watch_set();
        if set != self.watching {
            self.watching = set.clone();
            effects.push(Effect::Watch(set));
        }
    }

    /// Takes the file service's answer to `FileOp::Check`: a gone file puts every buffer
    /// visiting it in the deleted state, and a present one takes them out of it.
    pub(crate) fn on_checked(&mut self, files: Vec<(PathBuf, Option<u64>)>) {
        for (path, hash) in files {
            let visiting = self.buffers.iter_mut();
            for buffer in visiting.filter(|b| b.file.as_ref() == Some(&path)) {
                buffer.deleted = hash.is_none();
                if buffer.deleted {
                    buffer.disk = None;
                }
            }
        }
    }
}

impl Ctx<'_> {
    /// Checks the files visited at or under a reported path, every one on a rescan, and
    /// re-walks the indexes a create or delete made stale.
    pub(crate) fn on_files_changed(&mut self, paths: Vec<(PathBuf, Disk)>, rescan: bool) {
        let mut check: Vec<PathBuf> = (self.core.buffers.recent())
            .filter_map(|(_, b)| b.file.clone())
            .filter(|file| rescan || paths.iter().any(|(path, _)| file.starts_with(path)))
            .collect();
        check.sort();
        check.dedup();
        if !check.is_empty() {
            self.effects.push(Effect::File(FileOp::Check(check)));
        }
        let stale: Vec<String> = (self.core.index.iter_mut())
            .filter_map(|(name, index)| (stale(index, &paths) || rescan).then(|| name.clone()))
            .collect();
        for name in stale {
            // A project no longer loaded has nothing to walk.
            _ = self.index_project(&name, true);
        }
    }
}

/// Whether a reported path under one of the index's roots is a file it lacks, or is gone while
/// it holds that file or files under it. A file the index lacks starts a walk once, so writes
/// to a file the walk ignores (a visited `.env`) start none.
// shortcut: a create or delete re-walks the whole project; update the index per path if large projects lag.
fn stale(index: &mut FileIndex, paths: &[(PathBuf, Disk)]) -> bool {
    let mut stale = false;
    for (path, disk) in paths {
        for (root, files) in &index.roots {
            let Ok(rel) = path.strip_prefix(root) else {
                continue;
            };
            stale |= match disk {
                Disk::File => !files.iter().any(|f| f == rel) && index.missed.insert(path.clone()),
                Disk::Gone => {
                    index.missed.remove(path);
                    files.iter().any(|f| f.starts_with(rel))
                }
                Disk::Dir => false,
            };
        }
    }
    stale
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use crate::harness::{Harness, name};
    use crate::project::{Loaded, Project, Root};
    use crate::{Disk, Msg};

    /// Reports `names` in the temp dir as the watcher would, each as it is on disk now.
    fn report(h: &mut Harness, names: &[&str]) {
        let paths = (names.iter().map(|n| h.path(n)))
            .map(|p| {
                let disk = match std::fs::metadata(&p) {
                    Err(_) => Disk::Gone,
                    Ok(m) if m.is_dir() => Disk::Dir,
                    Ok(_) => Disk::File,
                };
                (p, disk)
            })
            .collect();
        h.send(Msg::FilesChanged {
            paths,
            rescan: false,
        });
    }

    fn mode_line(h: &mut Harness) -> String {
        h.display(5).mode_line
    }

    #[test]
    fn a_deleted_file_shows_in_the_mode_line_and_comes_back() {
        let mut h = Harness::new(&[("a.txt", "a\n")]);
        h.open("a.txt");
        std::fs::remove_file(h.path("a.txt")).unwrap();
        report(&mut h, &["a.txt"]);
        assert_eq!(mode_line(&mut h), "-- a.txt [deleted]  1:0");
        assert_eq!(h.text(), "a\n");
        let buffer = h.core.current();
        assert!(!buffer.modified() && buffer.disk.is_none());
        std::fs::write(h.path("a.txt"), "back\n").unwrap();
        report(&mut h, &["a.txt"]);
        assert_eq!(mode_line(&mut h), "-- a.txt  1:0");
        // Its hash is not taken: the file isn't the buffer's text, so a save is refused (#81 reverts).
        h.keys("x C-x C-s");
        assert_eq!(h.disk("a.txt"), "back\n");
    }

    #[test]
    fn a_deleted_or_renamed_directory_deletes_the_files_under_it() {
        let mut h = Harness::new(&[("d/a.txt", ""), ("e/b.txt", "")]);
        h.open("d/a.txt").open("e/b.txt");
        std::fs::remove_dir_all(h.path("d")).unwrap();
        std::fs::rename(h.path("e"), h.path("f")).unwrap();
        // inotify reports only the directories.
        report(&mut h, &["d", "e"]);
        assert!(mode_line(&mut h).contains("[deleted]"));
        h.keys("C-x b a.txt RET");
        assert!(mode_line(&mut h).contains("[deleted]"));
        assert!(h.core.buffers.visiting(&h.path("f/b.txt")).is_none());
    }

    #[test]
    fn a_rescan_checks_every_visited_file_in_every_workspace() {
        let mut h = Harness::new(&[("a.txt", ""), ("b.txt", "")]);
        h.open("a.txt");
        h.core.new_workspace("w", "home");
        h.open("b.txt");
        std::fs::remove_file(h.path("a.txt")).unwrap();
        h.send(Msg::FilesChanged {
            paths: Vec::new(),
            rescan: true,
        });
        let a = h.core.buffers.find("a.txt").unwrap();
        assert!(h.core.buffers.get(a).unwrap().deleted);
        assert!(!h.core.current().deleted);
    }

    #[test]
    fn a_clean_deleted_buffer_does_not_stop_quitting() {
        let mut h = Harness::new(&[("a.txt", "")]);
        h.open("a.txt");
        std::fs::remove_file(h.path("a.txt")).unwrap();
        report(&mut h, &["a.txt"]);
        h.keys("C-x C-c");
        assert!(h.quit);
    }

    #[test]
    fn saving_a_deleted_buffer_writes_it_back() {
        let mut h = Harness::new(&[("a.txt", "a\n")]);
        h.open("a.txt");
        std::fs::remove_file(h.path("a.txt")).unwrap();
        report(&mut h, &["a.txt"]);
        h.keys("C-x C-s");
        assert_eq!(h.disk("a.txt"), "a\n");
        assert_eq!(mode_line(&mut h), "-- a.txt  1:0");
        h.keys("C-x C-s");
        assert_eq!(h.core.message(), Some("(No changes need to be saved)"));
        // A delete the watcher missed: the save writes the file back too.
        std::fs::remove_file(h.path("a.txt")).unwrap();
        h.keys("x C-x C-s");
        assert_eq!(h.disk("a.txt"), "xa\n");
    }

    #[test]
    fn the_watch_set_follows_buffers_and_workspaces() {
        let mut h = Harness::new(&[("a/x", ""), ("out.txt", "")]);
        h.open("out.txt");
        h.keys("C-g");
        assert_eq!(
            h.watched.roots,
            Vec::<PathBuf>::new(),
            "$HOME isn't watched"
        );
        assert_eq!(h.watched.files, [h.path("out.txt")]);
        let project = Project {
            name: "p".into(),
            default_agent: None,
            primary: Root {
                path: h.path("a"),
                role: String::new(),
            },
            roots: Vec::new(),
        };
        h.core.projects.push(Loaded {
            project,
            file: None,
        });
        h.core.new_workspace("w", "p");
        h.keys("C-g");
        assert_eq!(h.watched.roots, [h.path("a")]);
        h.keys("C-x t 0");
        assert_eq!(h.watched.roots, Vec::<PathBuf>::new());
        h.keys("C-x k RET");
        assert_eq!(name(&h), "*scratch*<main>");
        assert_eq!(h.watched.files, Vec::<PathBuf>::new());
    }

    /// The `p` project on `a`, indexed, with the current workspace bound to it.
    fn indexed() -> Harness {
        let mut h = Harness::new(&[("a/x", "")]);
        h.core.projects.push(Loaded::ad_hoc("p", &h.path("a")));
        h.core.workspaces[0].project = "p".into();
        h.keys("C-x p i p RET");
        h.core.message = None;
        h
    }

    fn found(h: &mut Harness) -> Vec<String> {
        h.keys("C-x p f");
        let files = h.core.minibuffer().unwrap().candidates.clone();
        h.keys("C-g");
        files
    }

    #[test]
    fn creates_and_deletes_refresh_the_index_quietly() {
        let mut h = indexed();
        std::fs::write(h.path("a/y"), "").unwrap();
        report(&mut h, &["a/y"]);
        assert_eq!(h.core.message(), None);
        assert_eq!(found(&mut h), ["a/x", "a/y"]);
        std::fs::remove_file(h.path("a/x")).unwrap();
        report(&mut h, &["a/x"]);
        assert_eq!(found(&mut h), ["a/y"]);
        let walks = h.core.index_walks;
        std::fs::write(h.path("a/y"), "written").unwrap();
        report(&mut h, &["a/y"]);
        assert_eq!(h.core.index_walks, walks, "a write starts no walk");
        // A file the walk ignores (the harness's ignores only `.git`) starts one walk, then none.
        std::fs::create_dir(h.path("a/.git")).unwrap();
        std::fs::write(h.path("a/.git/x"), "").unwrap();
        report(&mut h, &["a/.git/x"]);
        report(&mut h, &["a/.git/x"]);
        assert_eq!(h.core.index_walks, walks + 1);
        assert_eq!(found(&mut h), ["a/y"]);
        // Lost events re-walk every index, quietly.
        h.core.message = None;
        h.send(Msg::FilesChanged {
            paths: Vec::new(),
            rescan: true,
        });
        assert_eq!((h.core.index_walks, h.core.message()), (walks + 2, None));
    }
}
