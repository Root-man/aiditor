//! Following the disk: what the watcher watches, and what its reports do to buffers and indexes.

use std::path::PathBuf;

use crate::command::Args;
use crate::file_index::scope;
use crate::project::{Entry, ProjectId};
use crate::{Core, Ctx, Disk, Effect, Effects, FileOp, WatchSet};

impl Core {
    /// A project with `watch = true`: its kept files and directories, as a walk of its current
    /// roots and patterns counted them without an error, and its `max_watched_files`.
    fn watch_counts(&self, entry: &Entry) -> Option<(usize, usize, usize)> {
        let project = &entry.project;
        if !project.watch {
            return None;
        }
        let (files, directories) = self.index.get(&entry.id)?.counts(&scope(project))?;
        let limit = project
            .max_watched_files
            .unwrap_or(self.settings.max_watched_files);
        Some((files, directories, limit))
    }

    pub(crate) fn project_watched(&self, entry: &Entry) -> bool {
        (self.watch_counts(entry)).is_some_and(|(files, dirs, limit)| files.max(dirs) <= limit)
    }

    /// The roots of every project a workspace is bound to, with its ignore patterns, and every
    /// file a buffer visits.
    pub fn watch_set(&self) -> WatchSet {
        let mut roots: Vec<(PathBuf, Vec<String>)> = (self.projects.iter())
            .filter(|p| self.workspaces.iter().any(|w| w.project == p.id))
            .filter(|p| self.project_watched(p))
            .flat_map(|p| {
                (p.project.all_roots()).map(|r| (r.path.clone(), p.project.ignore.clone()))
            })
            .collect();
        roots.sort();
        roots.dedup();
        let mut files: Vec<PathBuf> = (self.buffers.recent())
            .filter_map(|(_, b)| b.file.clone())
            .collect();
        files.sort();
        files.dedup();
        let directories = self
            .projects
            .iter()
            .filter(|p| self.workspaces.iter().any(|w| w.project == p.id))
            .filter(|p| self.project_watched(p))
            .map(|p| self.index[&p.id].directories().clone())
            .collect();
        WatchSet {
            roots,
            files,
            directories,
        }
    }

    /// Sends the watch set when it changed since last sent.
    pub(crate) fn watch(&mut self, effects: &mut Effects) {
        let refusals: std::collections::BTreeMap<_, _> = (self.projects.iter())
            .filter(|p| self.workspaces.iter().any(|w| w.project == p.id))
            .filter_map(|p| Some((p.id, self.watch_counts(p)?)))
            .filter(|(_, (files, dirs, limit))| files.max(dirs) > limit)
            .collect();
        for (name, &(files, directories, limit)) in &refusals {
            if self.watch_refusals.get(name) != Some(&(files, directories, limit)) {
                let name = self.project_name(*name);
                self.report(format!("Project {name} is unwatched: files = {files}, directories = {directories}; max_watched_files = {limit} exceeded"));
            }
        }
        self.watch_refusals = refusals;
        let set = self.watch_set();
        if std::mem::take(&mut self.watch_refresh) || set != self.watching {
            self.watching = set.clone();
            effects.push(Effect::Watch(set));
        }
    }
}

impl Ctx<'_> {
    pub(crate) fn ensure_watch_indexes(&mut self) {
        let names: Vec<_> = self
            .core
            .projects
            .iter()
            .filter(|p| {
                p.project.watch
                    && self
                        .core
                        .index
                        .get(&p.id)
                        .is_none_or(|i| !i.requested(&scope(&p.project)))
            })
            .filter(|p| self.core.workspaces.iter().any(|w| w.project == p.id))
            .map(|p| p.id)
            .collect();
        for name in names {
            _ = self.index_project(name, true);
        }
    }

    /// Takes the file service's answer to `FileOp::Check`: a gone file puts every buffer
    /// visiting it in the deleted state, and a present one takes them out of it, reverts the
    /// unmodified ones whose hash differs and marks the modified ones conflicted. An equal
    /// hash is the buffer's own save, or the file back as the buffer last had it.
    pub(crate) fn on_checked(&mut self, files: Vec<(PathBuf, Option<u64>)>) {
        for (path, hash) in files {
            let visiting = self.core.buffers.iter_mut();
            for (id, buffer) in visiting.filter(|(_, b)| b.file.as_ref() == Some(&path)) {
                buffer.deleted = hash.is_none();
                if buffer.deleted {
                    // Nothing on disk is lost by writing the buffer back, so there is no conflict.
                    (buffer.disk, buffer.conflict) = (None, None);
                } else if buffer.disk == hash {
                    buffer.conflict = None;
                } else if !buffer.modified() {
                    self.effects.push(Effect::File(FileOp::Revert {
                        buffer: id,
                        tick: buffer.tick(),
                        path: path.clone(),
                        text: buffer.text().clone(),
                    }));
                } else if buffer.conflict != hash {
                    (buffer.conflict, buffer.conflict_asked) = (hash, false);
                }
            }
        }
    }

    /// Asks how to resolve the current buffer's conflict when it has just been focused, once
    /// per disk version; a minibuffer being open defers the focus until it closes.
    pub(crate) fn ask_on_focus(&mut self) {
        if self.core.prompts.minibuffer.is_some() {
            return;
        }
        let id = self.core.windows.window.buffer();
        let focused = std::mem::replace(&mut self.core.focused, id);
        let buffer = self.core.current_mut();
        if focused != id && buffer.conflict.is_some() && !buffer.conflict_asked {
            buffer.conflict_asked = true;
            self.call("resolve-conflict", Args::default());
        }
    }

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
        let watched: std::collections::BTreeSet<_> = self
            .core
            .projects
            .iter()
            .filter(|p| self.core.project_watched(p))
            .map(|p| p.id)
            .collect();
        let stale: Vec<ProjectId> = (self.core.index.iter_mut())
            .filter(|(name, _)| watched.contains(*name))
            .filter_map(|(name, index)| {
                if rescan {
                    index.refresh_filtering();
                }
                (index.stale(&paths) || rescan).then_some(*name)
            })
            .collect();
        for name in stale {
            // A project no longer loaded has nothing to walk.
            _ = self.index_project(name, true);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use crate::buffer::hash;
    use crate::command::Args;
    use crate::harness::{Harness, name};
    use crate::project::{Loaded, Project, Root};
    use crate::{Disk, Effect, FileOp, Msg};

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
    fn watcher_status_reaches_echo_and_messages() {
        let mut h = Harness::new(&[]);
        for message in [
            "File watching failed: add refused; retrying",
            "File watching recovered",
        ] {
            h.send(Msg::WatchStatus(message.into()));
            assert!(h.core.message().unwrap().ends_with(message));
        }
        let messages = h
            .core
            .buffers
            .recent()
            .find(|(_, b)| b.name == "*Messages*")
            .unwrap()
            .1
            .text()
            .to_string();
        assert!(messages.contains("add refused"));
        assert!(messages.contains("File watching recovered"));
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
        // A clean buffer reverts to the file that came back, taking its hash.
        assert_eq!(mode_line(&mut h), "-- a.txt  1:0");
        assert_eq!(h.text(), "back\n");
        h.keys("x C-x C-s");
        assert_eq!(h.disk("a.txt"), "xback\n");
    }

    #[test]
    fn a_save_in_one_workspace_reverts_the_other_workspaces_buffer() {
        let mut h = Harness::new(&[("a.txt", "a\n")]);
        h.open("a.txt");
        let main = h.core.windows.window.buffer();
        h.new_workspace("w", "home");
        h.open("a.txt");
        h.keys("x C-x C-s");
        assert_eq!(h.disk("a.txt"), "xa\n");
        // The check the watcher's report of that save starts, answered by hand to see who reverts.
        let checked = vec![(h.path("a.txt"), Some(hash(b"xa\n")))];
        let effects = h.core.handle(h.now, Msg::Checked(checked));
        let reverted: Vec<_> = (effects.iter())
            .filter_map(|e| match e {
                Effect::File(FileOp::Revert { buffer, .. }) => Some(*buffer),
                _ => None,
            })
            .collect();
        assert_eq!(reverted, [main], "the saving buffer is left alone");
        h.run(effects);
        let buffer = h.core.buffers.get(main).unwrap();
        assert_eq!(
            (buffer.text().to_string(), buffer.modified()),
            ("xa\n".into(), false)
        );
        // Through the watcher's report too.
        std::fs::write(h.path("a.txt"), "xay\n").unwrap();
        report(&mut h, &["a.txt"]);
        let buffer = h.core.buffers.get(main).unwrap();
        assert_eq!(buffer.text().to_string(), "xay\n");
        assert_eq!(h.text(), "xay\n");
    }

    #[test]
    fn another_programs_write_reverts_a_clean_buffer_quietly() {
        let mut h = Harness::new(&[("a.txt", "a\nb\nc\n")]);
        h.open("a.txt");
        h.keys("C-n C-n C-f");
        h.core.message = None;
        std::fs::write(h.path("a.txt"), "new\na\nb\nc\n").unwrap();
        report(&mut h, &["a.txt"]);
        assert_eq!(h.text(), "new\na\nb\nc\n");
        assert_eq!(mode_line(&mut h), "-- a.txt  4:1");
        assert_eq!(h.core.message(), None);
        assert!(!h.core.current().modified());
        h.keys("C-/");
        assert_eq!(h.text(), "a\nb\nc\n");
    }

    #[test]
    fn a_failed_auto_revert_says_why() {
        let mut h = Harness::new(&[("a.txt", "a\n")]);
        h.open("a.txt");
        let path = h.path("a.txt");
        std::fs::write(&path, b"a\0").unwrap();
        report(&mut h, &["a.txt"]);
        assert_eq!(h.text(), "a\n");
        let binary = format!("{}: binary file", path.display());
        assert_eq!(h.core.message(), Some(binary.as_str()));
    }

    /// `a.txt` holding `a`, edited to `xa`, then rewritten to `other` by another program.
    fn conflicted() -> Harness {
        let mut h = Harness::new(&[("a.txt", "a\n")]);
        h.open("a.txt");
        h.keys("x");
        std::fs::write(h.path("a.txt"), "other\n").unwrap();
        report(&mut h, &["a.txt"]);
        h
    }

    fn prompt(h: &mut Harness) -> Option<String> {
        h.core.minibuffer().map(|m| m.prompt)
    }

    fn asked(h: &mut Harness) -> String {
        format!("{} changed on disk: ", h.path("a.txt").display())
    }

    #[test]
    fn a_modified_buffer_is_conflicted_not_reverted() {
        let mut h = conflicted();
        assert_eq!(h.text(), "xa\n");
        assert_eq!(mode_line(&mut h), "** a.txt [conflict]  1:1");
        assert_eq!(prompt(&mut h), None, "already focused, so not asked");
        h.keys("C-x C-s");
        assert_eq!(prompt(&mut h), Some(asked(&mut h)));
        let candidates = h.core.minibuffer().unwrap().candidates;
        assert_eq!(candidates, ["Compare", "Take disk", "Keep mine"]);
        h.keys("C-g");
        assert_eq!(h.disk("a.txt"), "other\n");
        assert!(mode_line(&mut h).contains("[conflict]"));
    }

    #[test]
    fn keep_mine_from_a_save_overwrites_the_disk() {
        let mut h = conflicted();
        h.keys("C-x C-s Keep SPC mine RET");
        assert_eq!(h.disk("a.txt"), "xa\n");
        assert_eq!(mode_line(&mut h), "-- a.txt  1:1");
    }

    #[test]
    fn keep_mine_from_m_x_waits_for_the_save() {
        let mut h = conflicted();
        h.keys("M-x resolve-conflict RET Keep SPC mine RET");
        assert_eq!(h.disk("a.txt"), "other\n");
        assert_eq!(mode_line(&mut h), "** a.txt  1:1");
        h.keys("C-x C-s");
        assert_eq!(h.disk("a.txt"), "xa\n");
    }

    #[test]
    fn an_answer_after_the_conflict_ended_does_nothing() {
        let mut h = conflicted();
        h.keys("M-x resolve-conflict RET");
        std::fs::write(h.path("a.txt"), "a\n").unwrap();
        report(&mut h, &["a.txt"]);
        h.keys("Keep SPC mine RET");
        assert_eq!(h.core.message(), None);
        assert_eq!(h.disk("a.txt"), "a\n");
    }

    #[test]
    fn take_disk_reverts_and_keeps_the_edits_in_undo() {
        let mut h = conflicted();
        h.keys("M-x resolve-conflict RET Take SPC disk RET");
        assert_eq!(h.text(), "other\n");
        assert_eq!(mode_line(&mut h), "-- a.txt  1:0");
        h.keys("C-/");
        assert_eq!(h.text(), "xa\n");
    }

    #[test]
    fn compare_shows_the_diff_and_stays_conflicted() {
        let mut h = conflicted();
        h.keys("C-x C-s RET");
        assert_eq!(name(&h), "a.txt");
        assert!(mode_line(&mut h).contains("[conflict]"));
        let other = *h.core.windows.others.keys().next().unwrap();
        let diff = h.core.windows.others[&other].buffer();
        let text = h.core.buffers.get(diff).unwrap().text().to_string();
        assert!(text.contains("-other\n+xa\n"), "{text}");
        assert_eq!(h.disk("a.txt"), "other\n");
    }

    #[test]
    fn focusing_a_conflicted_buffer_asks_once() {
        let mut h = conflicted();
        h.keys("C-x b RET");
        assert_eq!(name(&h), "*scratch*<main>");
        h.keys("C-x b RET");
        assert_eq!(prompt(&mut h), Some(asked(&mut h)));
        h.keys("C-g C-x b RET C-x b RET");
        assert_eq!(prompt(&mut h), None);
        assert_eq!(name(&h), "a.txt");
        // A new disk version asks again; keep mine then lets a plain save overwrite it.
        std::fs::write(h.path("a.txt"), "third\n").unwrap();
        report(&mut h, &["a.txt"]);
        h.keys("C-x b RET C-x b RET");
        assert_eq!(prompt(&mut h), Some(asked(&mut h)));
        h.keys("Keep SPC mine RET");
        assert_eq!(mode_line(&mut h), "** a.txt  1:1");
        h.keys("C-x C-s");
        assert_eq!(h.disk("a.txt"), "xa\n");
    }

    #[test]
    fn a_conflict_ends_when_the_file_goes_back_or_is_deleted() {
        let mut h = conflicted();
        std::fs::write(h.path("a.txt"), "a\n").unwrap();
        report(&mut h, &["a.txt"]);
        assert_eq!(mode_line(&mut h), "** a.txt  1:1");
        std::fs::write(h.path("a.txt"), "other\n").unwrap();
        report(&mut h, &["a.txt"]);
        std::fs::remove_file(h.path("a.txt")).unwrap();
        report(&mut h, &["a.txt"]);
        assert_eq!(mode_line(&mut h), "** a.txt [deleted]  1:1");
        h.keys("C-x C-s");
        assert_eq!(h.disk("a.txt"), "xa\n");
        h.keys("M-x resolve-conflict RET");
        let none = "No conflict with the file on disk";
        assert_eq!(h.core.message(), Some(none));
    }

    #[test]
    fn a_change_the_watcher_missed_conflicts_on_save() {
        let mut h = Harness::new(&[("a.txt", "a\n")]);
        h.open("a.txt");
        h.keys("x");
        std::fs::write(h.path("a.txt"), "other\n").unwrap();
        h.keys("C-x C-s");
        assert!(h.core.message().unwrap().contains("changed on disk"));
        assert!(mode_line(&mut h).contains("[conflict]"));
        h.keys("C-x C-s");
        assert_eq!(prompt(&mut h), Some(asked(&mut h)));
    }

    #[test]
    fn a_clean_buffer_of_a_missing_file_takes_the_created_file() {
        let mut h = Harness::new(&[]);
        h.open("new.txt");
        std::fs::write(h.path("new.txt"), "made\n").unwrap();
        report(&mut h, &["new.txt"]);
        assert_eq!(h.text(), "made\n");
        assert!(!h.core.current().modified());
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
        h.new_workspace("w", "home");
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
        assert_eq!(h.watched.roots, [], "$HOME isn't watched");
        assert_eq!(h.watched.files, [h.path("out.txt")]);
        let project = Project {
            name: "p".into(),
            watch: true,
            max_watched_files: None,
            default_agent: None,
            ignore: vec!["*.log".into()],
            primary: Root {
                path: h.path("a"),
                role: String::new(),
            },
            roots: Vec::new(),
        };
        h.core.add_project(Loaded {
            project,
            file: None,
        });
        h.new_workspace("w", "p");
        h.keys("C-g");
        assert_eq!(h.watched.roots, [(h.path("a"), vec!["*.log".into()])]);
        h.keys("C-x t 0");
        assert_eq!(h.watched.roots, []);
        h.keys("C-x k RET");
        assert_eq!(name(&h), "*scratch*<main>");
        assert_eq!(h.watched.files, Vec::<PathBuf>::new());
    }

    /// The `p` project on `a`, indexed, with the current workspace bound to it.
    fn indexed() -> Harness {
        let mut h = Harness::new(&[("a/x", "")]);
        h.core.add_project(Loaded::ad_hoc("p", &h.path("a")));
        h.core.workspaces[0].project = h.core.project_id("p").unwrap();
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
    fn startup_waits_for_counts_and_project_limit_overrides_settings() {
        let mut h = Harness::new(&[("a/x", ""), ("a/y", "")]);
        let mut project = Loaded::ad_hoc("p", &h.path("a"));
        project.project.max_watched_files = Some(2);
        h.core.add_project(project);
        h.core.workspaces[0].project = h.core.project_id("p").unwrap();
        h.core.settings.max_watched_files = 1;
        let effects = h.core.handle(h.now, Msg::Tick);
        assert_eq!(h.core.watch_set().roots, []);
        assert!(
            effects.iter().any(
                |e| matches!(e, Effect::File(FileOp::Index { project, .. }) if *project == h.core.project_id("p").unwrap())
            )
        );
        h.keys("C-x p f");
        h.run(effects);
        assert_eq!(h.core.minibuffer().unwrap().candidates, ["a/x", "a/y"]);
        h.keys("C-g");
        assert_eq!(h.watched.roots, [(h.path("a"), vec![])]);
        h.core
            .projects
            .last_mut()
            .unwrap()
            .project
            .max_watched_files = None;
        h.send(Msg::Tick);
        assert_eq!(h.watched.roots, []);
        let message =
            "Project p is unwatched: files = 2, directories = 1; max_watched_files = 1 exceeded";
        assert!(h.core.message().unwrap().ends_with(message));
        let messages = || {
            h.core
                .buffers
                .recent()
                .find(|(_, b)| b.name == "*Messages*")
                .unwrap()
                .1
                .text()
                .to_string()
        };
        assert_eq!(messages().matches(message).count(), 1);
        h.send(Msg::Tick);
        let messages = h
            .core
            .buffers
            .recent()
            .find(|(_, b)| b.name == "*Messages*")
            .unwrap()
            .1
            .text()
            .to_string();
        assert_eq!(messages.matches(message).count(), 1);
        assert!(
            h.frame(800., 600.).windows[0]
                .display
                .mode_line
                .ends_with("[unwatched]")
        );
        h.send(Msg::SettingsLoaded(Ok(Some(
            "max_watched_files = 2".into(),
        ))));
        assert_eq!(h.watched.roots, [(h.path("a"), vec![])]);
    }

    #[test]
    fn over_limit_projects_keep_file_watches_and_resume_after_manual_index() {
        let mut h = indexed();
        h.core.settings.max_watched_files = 1;
        h.open("a/x");
        std::fs::write(h.path("a/y"), "").unwrap();
        report(&mut h, &["a/y"]);
        assert_eq!(h.watched.roots, []);
        assert_eq!(h.watched.files, [h.path("a/x")]);
        let walks = h.core.index_requests();
        std::fs::write(h.path("a/x"), "changed").unwrap();
        report(&mut h, &["a/x"]);
        assert_eq!(h.text(), "changed");
        assert_eq!(h.core.index_requests(), walks);
        std::fs::remove_file(h.path("a/y")).unwrap();
        h.keys("C-x p i p RET");
        assert_eq!(h.watched.roots, [(h.path("a"), vec![])]);
    }

    #[test]
    fn kept_directories_and_additional_roots_share_the_project_limit() {
        let mut h = indexed();
        std::fs::create_dir_all(h.path("a/empty/nested")).unwrap();
        h.core.settings.max_watched_files = 2;
        report(&mut h, &["a/empty"]);
        assert_eq!(h.watched.roots, []);
        assert!(
            h.core
                .message()
                .unwrap()
                .contains("files = 1, directories = 3")
        );
        h.core.settings.max_watched_files = 3;
        h.send(Msg::Tick);
        assert_eq!(h.watched.roots, [(h.path("a"), vec![])]);
        std::fs::create_dir(h.path("b")).unwrap();
        let b = h.path("b");
        h.core
            .projects
            .last_mut()
            .unwrap()
            .project
            .roots
            .push(Root {
                path: b,
                role: String::new(),
            });
        h.send(Msg::Tick);
        assert_eq!(h.watched.roots, []);
        assert!(h.core.message().unwrap().contains("directories = 4"));
        let mut empty = Harness::new(&[]);
        empty.core.add_project(Loaded::ad_hoc("p", &empty.path("")));
        empty.core.workspaces[0].project = empty.core.project_id("p").unwrap();
        empty.core.settings.max_watched_files = 0;
        empty.send(Msg::Tick);
        assert!(
            empty
                .core
                .message()
                .unwrap()
                .contains("files = 0, directories = 1; max_watched_files = 0 exceeded")
        );
        empty.send(Msg::SettingsLoaded(Ok(Some(
            "max_watched_files = 1".into(),
        ))));
        assert_eq!(empty.watched.roots, [(empty.path(""), vec![])]);
    }

    #[test]
    fn edits_and_reloads_keep_root_watches_while_rewalking() {
        let mut h = indexed();
        let strings = vec!["p".into(), h.path("a").display().to_string(), "Main".into()];
        let mut ctx = h.ctx();
        ctx.call(
            "project-set-role",
            Args {
                strings,
                ..Args::default()
            },
        );
        let reload = Msg::ProjectsLoaded {
            dir: None,
            projects: Vec::new(),
        };
        for msg in [Msg::Tick, reload] {
            let effects = h.core.handle(h.now, msg);
            assert!(!effects.iter().any(|e| matches!(e, Effect::Watch(_))));
            assert!(
                !h.frame(800., 600.).windows[0]
                    .display
                    .mode_line
                    .contains("[unwatched]")
            );
            h.run(effects);
            assert_eq!(h.watched.roots, [(h.path("a"), vec![])]);
        }
        // An unchanged refusal is not reported again by a reload's re-walk.
        h.core.settings.max_watched_files = 0;
        h.send(Msg::Tick);
        assert!(h.core.message().unwrap().contains("max_watched_files = 0"));
        h.core.message = None;
        h.send(Msg::ProjectsLoaded {
            dir: None,
            projects: Vec::new(),
        });
        assert_eq!((h.core.message(), &h.watched.roots[..]), (None, &[][..]));
    }

    #[test]
    fn only_a_directory_without_indexed_files_starts_a_walk() {
        let mut h = indexed();
        let walks = h.core.index_requests();
        report(&mut h, &["a"]);
        assert_eq!(h.core.index_requests(), walks);
        std::fs::create_dir(h.path("a/empty")).unwrap();
        report(&mut h, &["a/empty"]);
        report(&mut h, &["a/empty"]);
        assert_eq!(h.core.index_requests(), walks + 1);
    }

    #[test]
    fn a_reload_rewalks_the_indexes_quietly() {
        let mut h = indexed();
        let walks = h.core.index_requests();
        h.send(Msg::ProjectsLoaded {
            dir: None,
            projects: Vec::new(),
        });
        assert_eq!(
            (h.core.index_requests(), h.core.message()),
            (walks + 1, None)
        );
        // A saved project edited by hand is walked with its new patterns.
        let mut q = Loaded::ad_hoc("q", &h.path("a"));
        q.file = Some((h.path("q.toml"), String::new()));
        h.core.add_project(q.clone());
        h.keys("C-x p i q RET");
        q.project.ignore = vec!["*.log".into()];
        let effects = (h.core).handle(
            std::time::Instant::now(),
            Msg::ProjectsLoaded {
                dir: None,
                projects: vec![Ok(q)],
            },
        );
        let mut ignores: Vec<_> = (effects.iter())
            .filter_map(|e| match e {
                Effect::File(FileOp::Index {
                    project, ignore, ..
                }) => Some((h.core.project_name(*project), ignore)),
                _ => None,
            })
            .collect();
        ignores.sort();
        assert_eq!(ignores, [("p", &vec![]), ("q", &vec!["*.log".to_string()])]);
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
        let walks = h.core.index_requests();
        std::fs::write(h.path("a/y"), "written").unwrap();
        report(&mut h, &["a/y"]);
        assert_eq!(h.core.index_requests(), walks, "a write starts no walk");
        // A file the walk ignores (the harness's ignores only `.git`) starts one walk, then none.
        std::fs::create_dir(h.path("a/.git")).unwrap();
        std::fs::write(h.path("a/.git/x"), "").unwrap();
        report(&mut h, &["a/.git/x"]);
        report(&mut h, &["a/.git/x"]);
        assert_eq!(h.core.index_requests(), walks + 1);
        assert_eq!(found(&mut h), ["a/y"]);
        // Lost events re-walk every index, quietly.
        h.core.message = None;
        h.send(Msg::FilesChanged {
            paths: Vec::new(),
            rescan: true,
        });
        assert_eq!(
            (h.core.index_requests(), h.core.message()),
            (walks + 2, None)
        );
    }

    #[test]
    fn unwatched_roots_keep_file_reverts_and_on_demand_indexing() {
        let mut h = indexed();
        h.open("a/x");
        h.keys("C-x p w p RET C-n RET");
        assert_eq!(h.watched.roots, []);
        assert_eq!(h.watched.files, [h.path("a/x")]);
        let walks = h.core.index_requests();
        std::fs::write(h.path("a/y"), "").unwrap();
        std::fs::write(h.path("a/x"), "changed").unwrap();
        report(&mut h, &["a/x", "a/y"]);
        assert_eq!(h.text(), "changed");
        assert_eq!(h.core.index_requests(), walks);
        assert_eq!(found(&mut h), ["a/x"]);
        h.keys("C-x p i p RET");
        assert_eq!(found(&mut h), ["a/x", "a/y"]);
        std::fs::write(h.path("a/z"), "").unwrap();
        h.send(Msg::ProjectsLoaded {
            dir: None,
            projects: Vec::new(),
        });
        assert_eq!(found(&mut h), ["a/x", "a/y", "a/z"]);
        h.keys("C-x p w p RET C-p RET");
        assert_eq!(h.watched.roots, [(h.path("a"), vec![])]);
        std::fs::write(h.path("a/new"), "").unwrap();
        report(&mut h, &["a/new"]);
        assert_eq!(found(&mut h), ["a/new", "a/x", "a/y", "a/z"]);
    }

    #[test]
    fn shared_root_and_rescan_reports_refresh_only_watched_indexes() {
        let mut h = indexed();
        h.keys("C-x p w p RET C-n RET");
        h.core.add_project(Loaded::ad_hoc("q", &h.path("a")));
        h.new_workspace("q", "q");
        h.keys("C-x p i q RET");
        assert_eq!(h.watched.roots, [(h.path("a"), vec![])]);
        let walks = h.core.index_requests();
        std::fs::write(h.path("a/y"), "").unwrap();
        report(&mut h, &["a/y"]);
        assert_eq!(h.core.index_requests(), walks + 1);
        assert_eq!(found(&mut h), ["a/x", "a/y"]);
        std::fs::write(h.path("a/z"), "").unwrap();
        h.send(Msg::FilesChanged {
            paths: Vec::new(),
            rescan: true,
        });
        assert_eq!(h.core.index_requests(), walks + 2);
        assert_eq!(found(&mut h), ["a/x", "a/y", "a/z"]);
        h.keys("C-x p p p RET");
        assert_eq!(found(&mut h), ["a/x"]);
    }
}
