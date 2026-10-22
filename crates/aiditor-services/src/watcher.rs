//! The file watcher: debounced reports of the paths that may have changed under the watched
//! roots and the visited files' directories. Event kinds are not trusted; a path means "check
//! this path".

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::time::{Duration, Instant};

use aiditor_core::{Disk, Msg, WatchSet};
use notify::{EventKind, RecommendedWatcher, RecursiveMode, Watcher as _};

use crate::files::walker;

/// A report waits until no change has come for `QUIET`, and at most `MAX` after its first one.
const QUIET: Duration = Duration::from_millis(100);
const MAX: Duration = Duration::from_secs(1);

enum In {
    Set(WatchSet),
    Event(notify::Result<notify::Event>),
}

/// The watcher service's thread, told what to watch with `set`; it lives as long as the app.
pub struct Watcher(Sender<In>);

impl Watcher {
    /// Starts the service; each report goes to `send` as `Msg::FilesChanged`.
    pub fn spawn(send: impl Fn(Msg) + Send + 'static) -> Self {
        let (tx, rx) = mpsc::channel();
        let events = tx.clone();
        std::thread::spawn(move || {
            match notify::recommended_watcher(move |e| _ = events.send(In::Event(e))) {
                Ok(watcher) => run(State::new(watcher), rx, send),
                Err(err) => tracing::warn!(%err, "no file watcher; buffers won't follow the disk"),
            }
        });
        Watcher(tx)
    }

    /// Watches `set` from now on, instead of the set before.
    pub fn set(&self, set: WatchSet) {
        _ = self.0.send(In::Set(set));
    }
}

/// Collects event paths and sends them once quiet, applying watch sets as they come.
fn run(mut state: State, rx: Receiver<In>, send: impl Fn(Msg)) {
    let (mut pending, mut rescan) = (BTreeSet::new(), false);
    // When the pending report's first and last events came.
    let mut times: Option<(Instant, Instant)> = None;
    loop {
        let msg = match times.map(|(first, last)| (last + QUIET).min(first + MAX)) {
            Some(due) if Instant::now() >= due => {
                if let Some(msg) = state.flush(std::mem::take(&mut pending), rescan) {
                    send(msg);
                }
                (rescan, times) = (false, None);
                continue;
            }
            Some(due) => match rx.recv_timeout(due - Instant::now()) {
                Ok(msg) => msg,
                Err(RecvTimeoutError::Timeout) => continue,
                Err(RecvTimeoutError::Disconnected) => return,
            },
            None => match rx.recv() {
                Ok(msg) => msg,
                Err(_) => return,
            },
        };
        let event = match msg {
            In::Set(set) => {
                state.set(set);
                continue;
            }
            In::Event(Ok(event)) => event,
            In::Event(Err(err)) => {
                tracing::info!(%err, "watch error, rescanning");
                rescan = true;
                notify::Event::default()
            }
        };
        // inotify reports opens and reads, the editor's own checks among them.
        if matches!(event.kind, EventKind::Access(_)) && !event.need_rescan() {
            continue;
        }
        rescan |= event.need_rescan();
        pending.extend(event.paths.into_iter().map(|p| state.translate(p)));
        let now = Instant::now();
        times = Some((times.map_or(now, |(first, _)| first), now));
    }
}

/// What is watched, and how.
struct State {
    watcher: RecommendedWatcher,
    /// Each watched root's kept directories, the root included: those the index's walk enters.
    roots: BTreeMap<PathBuf, BTreeSet<PathBuf>>,
    /// The visited files.
    files: Vec<PathBuf>,
    /// The visited files' directories that no root covers.
    extra: BTreeSet<PathBuf>,
    /// What `notify` watches.
    watched: BTreeMap<PathBuf, RecursiveMode>,
    /// Watched paths' canonical forms that differ, with the path as watched: FSEvents reports
    /// canonical paths (`/private/var/...` for `/var/...`).
    aliases: Vec<(PathBuf, PathBuf)>,
}

impl State {
    fn new(watcher: RecommendedWatcher) -> Self {
        State {
            watcher,
            roots: BTreeMap::new(),
            files: Vec::new(),
            extra: BTreeSet::new(),
            watched: BTreeMap::new(),
            aliases: Vec::new(),
        }
    }

    fn set(&mut self, set: WatchSet) {
        self.roots.retain(|root, _| set.roots.contains(root));
        for root in set.roots {
            self.roots.entry(root).or_insert_with_key(|root| {
                (walker(root).build().flatten())
                    .filter(|e| e.file_type().is_some_and(|t| t.is_dir()))
                    .map(ignore::DirEntry::into_path)
                    .collect()
            });
        }
        self.files = set.files;
        self.sync();
    }

    /// Whether some root's walk enters `dir`.
    fn kept(&self, dir: &Path) -> bool {
        self.roots.values().any(|dirs| dirs.contains(dir))
    }

    /// Whether a change at `path` is reported: one in a kept or extra directory, or of a kept one.
    fn keep(&self, path: &Path) -> bool {
        self.kept(path)
            || (path.parent()).is_some_and(|dir| self.kept(dir) || self.extra.contains(dir))
    }

    /// Updates the extra directories and the `notify` watches to match the roots and files.
    fn sync(&mut self) {
        self.extra = (self.files.iter())
            .filter_map(|file| file.parent())
            .filter(|dir| !self.kept(dir))
            .map(Path::to_path_buf)
            .collect();
        let mut want = BTreeMap::new();
        for (root, dirs) in &self.roots {
            match cfg!(target_os = "macos") {
                true => _ = want.insert(root.clone(), RecursiveMode::Recursive),
                false => want.extend(
                    dirs.iter()
                        .map(|d| (d.clone(), RecursiveMode::NonRecursive)),
                ),
            }
        }
        for dir in &self.extra {
            want.entry(dir.clone())
                .or_insert(RecursiveMode::NonRecursive);
        }
        let mut paths = self.watcher.paths_mut();
        // Errors are expected for directories deleted since; their events are gone anyway.
        for (path, mode) in &self.watched {
            if want.get(path) != Some(mode)
                && let Err(err) = paths.remove(path)
            {
                tracing::info!(%err, path = %path.display(), "unwatch failed");
            }
        }
        for (path, &mode) in &want {
            if self.watched.get(path) != Some(&mode)
                && let Err(err) = paths.add(path, mode)
            {
                tracing::info!(%err, path = %path.display(), "watch failed");
            }
        }
        if let Err(err) = paths.commit() {
            tracing::info!(%err, "watch failed");
        }
        self.watched = want;
        // inotify reports paths as watched, so only FSEvents needs them mapped back.
        if cfg!(target_os = "macos") {
            self.aliases = (self.watched.keys())
                .filter_map(|path| {
                    let canonical = fs::canonicalize(path).ok()?;
                    (canonical != *path).then(|| (canonical, path.clone()))
                })
                .collect();
        }
    }

    /// `path` as the core knows it.
    fn translate(&self, path: PathBuf) -> PathBuf {
        for (canonical, watched) in &self.aliases {
            if let Ok(rest) = path.strip_prefix(canonical) {
                return watched.join(rest);
            }
        }
        path
    }

    /// The report for the event paths `raw`, if there is anything to report. A new kept
    /// directory is watched and its files reported; a gone one stops being watched.
    fn flush(&mut self, raw: BTreeSet<PathBuf>, rescan: bool) -> Option<Msg> {
        // Each kept directory's entries the index's walk keeps, listed once per report; listing
        // the parent applies the ignore files above it, as the index's walk does.
        let mut listings: BTreeMap<PathBuf, BTreeSet<PathBuf>> = BTreeMap::new();
        let mut listed = |parent: &Path, path: &Path| {
            (listings.entry(parent.to_path_buf()))
                .or_insert_with(|| {
                    let listing = walker(parent).max_depth(Some(1)).build().flatten();
                    listing.map(ignore::DirEntry::into_path).collect()
                })
                .contains(path)
        };
        let mut report = BTreeSet::new();
        for path in raw.iter().filter(|p| self.keep(p)) {
            // An ignored file in a kept directory is left out, unless a buffer visits it.
            let ignored = match path.parent().filter(|dir| self.kept(dir)) {
                Some(dir) if !self.files.contains(path) && fs::symlink_metadata(path).is_ok() => {
                    !listed(dir, path)
                }
                _ => false,
            };
            if !ignored {
                report.insert(path.clone());
            }
        }
        let mut changed = false;
        for path in &raw {
            let dir = fs::symlink_metadata(path).is_ok_and(|m| m.is_dir());
            let parent = path.parent().filter(|parent| self.kept(parent));
            if dir && let Some(parent) = parent.filter(|_| !self.kept(path)) {
                if !listed(parent, path) {
                    continue;
                }
                // Files made before the watch was in place are reported.
                for entry in walker(path).build().flatten() {
                    match entry.file_type().is_some_and(|t| t.is_dir()) {
                        true => (self.roots.values_mut())
                            .filter(|dirs| dirs.contains(parent))
                            .for_each(|dirs| _ = dirs.insert(entry.path().to_path_buf())),
                        false => _ = report.insert(entry.into_path()),
                    }
                }
                changed = true;
            } else if !dir && self.kept(path) && !self.roots.contains_key(path) {
                for dirs in self.roots.values_mut() {
                    dirs.retain(|d| !d.starts_with(path));
                }
                changed = true;
            }
        }
        if changed {
            self.sync();
        }
        if report.is_empty() && !rescan {
            return None;
        }
        let paths = (report.into_iter())
            .map(|path| {
                let disk = match fs::metadata(&path) {
                    Err(_) => Disk::Gone,
                    Ok(m) if m.is_dir() => Disk::Dir,
                    Ok(_) => Disk::File,
                };
                (path, disk)
            })
            .collect();
        Some(Msg::FilesChanged { paths, rescan })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A temp dir holding `files`, as (relative path, contents).
    fn tree(files: &[(&str, &str)]) -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        for (name, text) in files {
            let path = dir.path().join(name);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, text).unwrap();
        }
        dir
    }

    fn set(roots: &[&Path], files: &[&Path]) -> WatchSet {
        WatchSet {
            roots: roots.iter().map(|r| r.to_path_buf()).collect(),
            files: files.iter().map(|f| f.to_path_buf()).collect(),
        }
    }

    /// The service watching `set`, once its watches are in place: a probe file in `probe` (a
    /// watched directory) is written until a report comes, then reports settle.
    fn start(set: WatchSet, probe: &Path) -> (Watcher, Receiver<Msg>) {
        let (tx, rx) = mpsc::channel();
        let watcher = Watcher::spawn(move |msg| _ = tx.send(msg));
        watcher.set(set);
        let probe = probe.join(".probe");
        for i in 0.. {
            assert!(i < 50, "the watcher never reported");
            fs::write(&probe, i.to_string()).unwrap();
            if rx.recv_timeout(Duration::from_millis(200)).is_ok() {
                break;
            }
        }
        fs::remove_file(&probe).unwrap();
        settle(&rx);
        (watcher, rx)
    }

    /// Drops reports until none comes for half a second.
    fn settle(rx: &Receiver<Msg>) {
        while rx.recv_timeout(Duration::from_millis(500)).is_ok() {}
    }

    /// The next report's paths, waiting up to 5 s.
    fn next(rx: &Receiver<Msg>) -> Vec<(PathBuf, Disk)> {
        match rx.recv_timeout(Duration::from_secs(5)) {
            Ok(Msg::FilesChanged { paths, .. }) => paths,
            other => panic!("no report: {other:?}"),
        }
    }

    #[test]
    fn one_write_makes_one_report() {
        let dir = tree(&[("a.txt", "a")]);
        let a = dir.path().join("a.txt");
        let (_w, rx) = start(set(&[dir.path()], &[]), dir.path());
        fs::write(&a, "b").unwrap();
        assert_eq!(next(&rx), [(a, Disk::File)]);
        assert!(rx.recv_timeout(Duration::from_millis(500)).is_err());
    }

    #[test]
    fn a_steady_stream_of_changes_is_still_reported() {
        let dir = tree(&[("a.txt", "")]);
        let a = dir.path().join("a.txt");
        let (_w, rx) = start(set(&[dir.path()], &[]), dir.path());
        let writer = std::thread::spawn(move || {
            for i in 0..100 {
                fs::write(&a, i.to_string()).unwrap();
                std::thread::sleep(Duration::from_millis(30));
            }
        });
        // Writes go on for 3 s, never quiet for 100 ms; the cap sends a report within about 1 s.
        assert!(rx.recv_timeout(Duration::from_secs(2)).is_ok());
        writer.join().unwrap();
    }

    #[test]
    fn creates_deletes_and_an_atomic_save_are_reported() {
        let dir = tree(&[("a.txt", "a")]);
        let (a, b) = (dir.path().join("a.txt"), dir.path().join("b.txt"));
        let (_w, rx) = start(set(&[dir.path()], &[&a]), dir.path());
        fs::write(&b, "").unwrap();
        assert_eq!(next(&rx), [(b.clone(), Disk::File)]);
        fs::remove_file(&b).unwrap();
        assert_eq!(next(&rx), [(b, Disk::Gone)]);
        // Another program's atomic save: a temp file renamed over the file.
        let tmp = dir.path().join(".a.txt.tmp");
        fs::write(&tmp, "saved").unwrap();
        fs::rename(&tmp, &a).unwrap();
        assert!(next(&rx).contains(&(a.clone(), Disk::File)));
        // The file is still watched afterwards.
        settle(&rx);
        fs::write(&a, "again").unwrap();
        assert_eq!(next(&rx), [(a, Disk::File)]);
    }

    #[test]
    fn a_file_outside_the_roots_is_watched_through_its_directory() {
        let (root, out) = (tree(&[("x", "")]), tree(&[("o.txt", "")]));
        let o = out.path().join("o.txt");
        let (w, rx) = start(set(&[root.path()], &[&o]), out.path());
        fs::remove_file(&o).unwrap();
        assert_eq!(next(&rx), [(o.clone(), Disk::Gone)]);
        // The last buffer visiting it goes, and so does the watch.
        w.set(set(&[root.path()], &[]));
        settle(&rx);
        fs::write(&o, "").unwrap();
        assert!(rx.recv_timeout(Duration::from_millis(500)).is_err());
    }

    /// A state with no events delivered, for checking what it watches.
    fn state() -> State {
        State::new(notify::recommended_watcher(|_| {}).unwrap())
    }

    #[test]
    fn extra_directory_watches_come_and_go_with_the_visited_files() {
        let (root, out) = (
            tree(&[(".gitignore", "target/\n"), ("target/t", "")]),
            tree(&[("a", ""), ("b", "")]),
        );
        let (a, b, t) = (
            out.path().join("a"),
            out.path().join("b"),
            root.path().join("target/t"),
        );
        let mut s = state();
        s.set(set(&[root.path()], &[&a, &b, &t]));
        let extra = |s: &State| s.extra.iter().cloned().collect::<Vec<_>>();
        // A file under an ignored directory is watched through its directory too.
        let mut both = [out.path().to_path_buf(), root.path().join("target")];
        both.sort();
        assert_eq!(extra(&s), both);
        assert!(s.watched.contains_key(out.path()));
        s.set(set(&[root.path()], &[&b]));
        assert!(s.watched.contains_key(out.path()), "b still visits it");
        s.set(set(&[root.path()], &[]));
        assert!(!s.watched.contains_key(out.path()));
        assert_eq!(extra(&s), Vec::<PathBuf>::new());
    }

    #[test]
    fn ignored_directories_get_no_watch_and_no_reports() {
        let dir = tree(&[
            (".gitignore", "node_modules/\n"),
            ("node_modules/pkg/x.js", ""),
            ("src/a.rs", ""),
        ]);
        let root = dir.path();
        let mut s = state();
        s.set(set(&[root], &[]));
        assert_eq!(
            s.roots[root].iter().cloned().collect::<Vec<_>>(),
            [root.to_path_buf(), root.join("src")]
        );
        if cfg!(target_os = "linux") {
            assert_eq!(
                s.watched.keys().cloned().collect::<Vec<_>>(),
                [root.to_path_buf(), root.join("src")]
            );
        }
        let (_w, rx) = start(set(&[root], &[]), root);
        fs::write(root.join("node_modules/pkg/x.js"), "changed").unwrap();
        let a = root.join("src/a.rs");
        fs::write(&a, "fence").unwrap();
        assert_eq!(next(&rx), [(a, Disk::File)]);
    }

    #[test]
    fn ignored_files_are_left_out_unless_visited() {
        let dir = tree(&[
            (".gitignore", "*.log\n"),
            ("a.log", ""),
            ("b.log", ""),
            ("c", ""),
        ]);
        let root = dir.path();
        let mut s = state();
        s.set(set(&[root], &[&root.join("b.log")]));
        let raw = ["a.log", "b.log", "c", "gone.log"].map(|n| root.join(n));
        let Some(Msg::FilesChanged { paths, rescan }) = s.flush(raw.into(), false) else {
            panic!("no report");
        };
        let want = [
            ("b.log", Disk::File),
            ("c", Disk::File),
            ("gone.log", Disk::Gone),
        ];
        assert_eq!(paths, want.map(|(n, d)| (root.join(n), d)));
        assert!(!rescan);
        assert_eq!(s.flush([root.join("a.log")].into(), false), None);
    }

    #[test]
    fn lost_events_are_reported_with_nothing_else() {
        let dir = tree(&[("c", "")]);
        let mut s = state();
        s.set(set(&[dir.path()], &[]));
        assert_eq!(
            s.flush(BTreeSet::new(), true),
            Some(Msg::FilesChanged {
                paths: Vec::new(),
                rescan: true
            })
        );
    }

    #[test]
    fn a_gone_directory_stops_being_kept_and_comes_back() {
        let dir = tree(&[("d/e/f", "")]);
        let (root, d) = (dir.path(), dir.path().join("d"));
        let mut s = state();
        s.set(set(&[root], &[]));
        fs::remove_dir_all(&d).unwrap();
        let gone = s.flush([d.clone()].into(), false);
        assert_eq!(
            gone,
            Some(Msg::FilesChanged {
                paths: vec![(d.clone(), Disk::Gone)],
                rescan: false
            })
        );
        assert_eq!(s.roots[root].iter().collect::<Vec<_>>(), [root]);
        fs::create_dir_all(d.join("e")).unwrap();
        fs::write(d.join("e/f"), "").unwrap();
        let Some(Msg::FilesChanged { paths, .. }) = s.flush([d.clone()].into(), false) else {
            panic!("no report");
        };
        assert!(paths.contains(&(d.join("e/f"), Disk::File)), "{paths:?}");
        assert!(s.kept(&d.join("e")));
    }

    #[test]
    fn a_new_directory_is_scanned_and_watched() {
        let (root, out) = (tree(&[("x", "")]), tree(&[("new/f", "made before")]));
        let (_w, rx) = start(set(&[root.path()], &[]), root.path());
        let new = root.path().join("new");
        fs::rename(out.path().join("new"), &new).unwrap();
        let f = new.join("f");
        assert!(next(&rx).contains(&(f.clone(), Disk::File)));
        settle(&rx);
        fs::write(&f, "later").unwrap();
        assert_eq!(next(&rx), [(f, Disk::File)]);
    }
}
