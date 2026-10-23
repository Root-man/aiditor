//! The file watcher: debounced reports of the paths that may have changed under the watched
//! roots and the visited files' directories. Event kinds are not trusted; a path means "check
//! this path".

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::time::{Duration, Instant};

use aiditor_core::{Disk, Msg, WatchSet};
use notify::{EventKind, RecommendedWatcher, RecursiveMode};

use crate::files::{Patterns, walker};

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
            let factory = move || {
                let events = events.clone();
                notify::recommended_watcher(move |e| _ = events.send(In::Event(e)))
                    .map(|w| Box::new(w) as Box<dyn Backend>)
            };
            run(State::with_factory(Box::new(factory)), rx, send);
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
        for message in state.messages.drain(..) {
            send(Msg::WatchStatus(message));
        }
        let now = Instant::now();
        if state.retry.is_some_and(|due| now >= due) {
            state.sync_at(now);
            continue;
        }
        let flush_due = times.map(|(first, last)| (last + QUIET).min(first + MAX));
        if flush_due.is_some_and(|due| now >= due) {
            if let Some(msg) = state.flush(std::mem::take(&mut pending), rescan) {
                send(msg);
            }
            (rescan, times) = (false, None);
            continue;
        }
        let due = flush_due.into_iter().chain(state.retry).min();
        let msg = match due {
            Some(due) => match rx.recv_timeout(due.saturating_duration_since(Instant::now())) {
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
        let paths: Vec<_> = event
            .paths
            .into_iter()
            .map(|p| state.translate(p))
            .collect();
        if matches!(
            event.kind,
            EventKind::Remove(_) | EventKind::Modify(notify::event::ModifyKind::Name(_))
        ) {
            for path in &paths {
                rescan |= state.kept(path);
                state.invalidate(path);
            }
        }
        pending.extend(paths);
        let now = Instant::now();
        times = Some((times.map_or(now, |(first, _)| first), now));
    }
}

fn is_ignore(path: &Path) -> bool {
    matches!(
        path.file_name().and_then(|s| s.to_str()),
        Some(".ignore" | ".gitignore")
    )
}

/// A watched root with its project's ignore patterns; a root shared by projects with different
/// patterns is one of these per project.
struct Root {
    path: PathBuf,
    patterns: Patterns,
    /// The kept directories, the root included: those the index's walk enters.
    dirs: BTreeSet<PathBuf>,
}

/// Each listed directory's entries that a root's walk keeps, by root index and directory.
type Listings = BTreeMap<(usize, PathBuf), BTreeSet<PathBuf>>;

trait Backend {
    fn paths_mut(&mut self) -> Box<dyn notify::PathsMut + '_>;
}

impl Backend for RecommendedWatcher {
    fn paths_mut(&mut self) -> Box<dyn notify::PathsMut + '_> {
        notify::Watcher::paths_mut(self)
    }
}

type Factory = Box<dyn FnMut() -> notify::Result<Box<dyn Backend>>>;

#[derive(Clone, Copy, Debug, PartialEq)]
struct Registration {
    mode: RecursiveMode,
    identity: (u64, u64),
}

fn identity(path: &Path) -> std::io::Result<(u64, u64)> {
    let meta = fs::metadata(path)?;
    if !meta.is_dir() {
        return Err(std::io::Error::other("not a directory"));
    }
    Ok((meta.dev(), meta.ino()))
}

/// Desired coverage and registrations acknowledged by the backend are kept separately.
struct State {
    factory: Factory,
    watcher: Option<Box<dyn Backend>>,
    roots: Vec<Root>,
    files: Vec<PathBuf>,
    extra: BTreeSet<PathBuf>,
    watched: BTreeMap<PathBuf, Registration>,
    uncertain: BTreeSet<PathBuf>,
    aliases: Vec<(PathBuf, PathBuf)>,
    retry: Option<Instant>,
    attempts: u32,
    failure: Option<String>,
    messages: Vec<String>,
}

impl State {
    #[cfg(test)]
    fn new(watcher: RecommendedWatcher) -> Self {
        let mut state = Self::with_factory(Box::new(|| unreachable!("test watcher supplied")));
        state.watcher = Some(Box::new(watcher));
        state
    }

    fn with_factory(factory: Factory) -> Self {
        State {
            factory,
            watcher: None,
            roots: Vec::new(),
            files: Vec::new(),
            extra: BTreeSet::new(),
            watched: BTreeMap::new(),
            uncertain: BTreeSet::new(),
            aliases: Vec::new(),
            retry: None,
            attempts: 0,
            failure: None,
            messages: Vec::new(),
        }
    }

    fn set(&mut self, set: WatchSet) {
        self.roots = set
            .roots
            .into_iter()
            .map(|(path, ignore)| {
                let patterns = Patterns::lenient(&ignore);
                let dirs = set
                    .directories
                    .iter()
                    .flat_map(|dirs| dirs.iter())
                    .filter(|d| d.starts_with(&path))
                    .cloned()
                    .collect();
                Root {
                    path,
                    patterns,
                    dirs,
                }
            })
            .collect();
        self.files = set.files;
        self.sync();
    }

    /// Whether some root's walk enters `dir`.
    fn kept(&self, dir: &Path) -> bool {
        self.roots.iter().any(|r| r.dirs.contains(dir))
    }

    /// Whether root `i` keeps `dir` and its walk, listing `dir`, keeps `path`; listing the
    /// directory applies the ignore files above it, as the index's walk does.
    fn listed(&self, listings: &mut Listings, i: usize, dir: &Path, path: &Path) -> bool {
        let root = &self.roots[i];
        root.dirs.contains(dir)
            && (listings.entry((i, dir.to_path_buf())))
                .or_insert_with(|| {
                    let listing = walker(dir, &root.path, &root.patterns)
                        .max_depth(Some(1))
                        .build();
                    (listing.flatten().map(ignore::DirEntry::into_path)).collect()
                })
                .contains(path)
    }

    /// Whether a change at `path` is reported: one in a kept or extra directory, or of a kept one.
    fn keep(&self, path: &Path) -> bool {
        (is_ignore(path) && self.roots.iter().any(|r| path.starts_with(&r.path)))
            || self.kept(path)
            || (path.parent()).is_some_and(|dir| self.kept(dir) || self.extra.contains(dir))
    }

    fn invalidate(&mut self, path: &Path) {
        let invalid: Vec<_> = self
            .watched
            .keys()
            .filter(|p| p.starts_with(path))
            .cloned()
            .collect();
        for path in invalid {
            self.watched.remove(&path);
            self.uncertain.insert(path);
        }
    }

    fn covered(&self, dir: &Path) -> bool {
        self.roots.iter().any(|root| {
            if !root.dirs.contains(dir) {
                return false;
            }
            let (path, mode) = if cfg!(target_os = "macos") {
                (root.path.as_path(), RecursiveMode::Recursive)
            } else {
                (dir, RecursiveMode::NonRecursive)
            };
            self.watched
                .get(path)
                .is_some_and(|r| r.mode == mode && identity(path).ok() == Some(r.identity))
        })
    }

    fn fallback(&self) -> BTreeSet<PathBuf> {
        self.files
            .iter()
            .filter_map(|file| file.parent())
            .filter(|dir| !self.covered(dir))
            .map(Path::to_path_buf)
            .collect()
    }

    fn sync(&mut self) {
        self.sync_at(Instant::now());
    }

    fn sync_at(&mut self, now: Instant) {
        if self.roots.is_empty() && self.files.is_empty() {
            self.watcher = None;
            self.watched.clear();
            self.uncertain.clear();
            self.extra.clear();
            self.aliases.clear();
            self.retry = None;
            self.attempts = 0;
            self.failure = None;
            return;
        }
        let mut errors = Vec::new();
        if self.watcher.is_none() {
            match (self.factory)() {
                Ok(watcher) => self.watcher = Some(watcher),
                Err(err) => errors.push(format!("initialization: {err}")),
            }
        }
        if self.watcher.is_some() {
            let mut roots = BTreeMap::new();
            for root in &self.roots {
                if cfg!(target_os = "macos") {
                    roots.insert(root.path.clone(), RecursiveMode::Recursive);
                } else {
                    roots.extend(
                        root.dirs
                            .iter()
                            .map(|d| (d.clone(), RecursiveMode::NonRecursive)),
                    );
                }
            }
            let mut want = roots.clone();
            for dir in self.fallback() {
                want.entry(dir).or_insert(RecursiveMode::NonRecursive);
            }
            self.apply(&want, &mut errors);
            self.extra = self.fallback();
            for dir in &self.extra {
                // A failed recursive root can still acknowledge a non-recursive fallback.
                roots.insert(dir.clone(), RecursiveMode::NonRecursive);
            }
            if roots != want {
                self.apply(&roots, &mut errors);
            }
            if cfg!(target_os = "macos") {
                self.aliases = self
                    .watched
                    .keys()
                    .filter_map(|path| {
                        let canonical = fs::canonicalize(path).ok()?;
                        (canonical != *path).then(|| (canonical, path.clone()))
                    })
                    .collect();
            }
        }
        errors.sort();
        errors.dedup();
        if errors.is_empty() {
            self.retry = None;
            self.attempts = 0;
            if self.failure.take().is_some() {
                self.messages.push("File watching recovered".into());
            }
        } else {
            let error = errors.join("; ");
            if self.failure.as_ref() != Some(&error) {
                self.messages
                    .push(format!("File watching failed: {error}; retrying"));
                self.failure = Some(error);
            }
            self.retry = Some(now + Duration::from_secs((1u64 << self.attempts.min(5)).min(30)));
            self.attempts = self.attempts.saturating_add(1);
        }
    }

    fn apply(&mut self, want: &BTreeMap<PathBuf, RecursiveMode>, errors: &mut Vec<String>) {
        let mut desired = BTreeMap::new();
        for (path, &mode) in want {
            match identity(path) {
                Ok(identity) => {
                    desired.insert(path.clone(), Registration { mode, identity });
                }
                Err(err) => errors.push(format!("{}: {err}", path.display())),
            }
        }
        let replaced: Vec<_> = self
            .watched
            .iter()
            .filter(|(path, old)| identity(path).ok() != Some(old.identity))
            .map(|(p, _)| p.clone())
            .collect();
        for path in replaced {
            self.invalidate(&path);
        }
        let remove: BTreeSet<_> = self
            .watched
            .iter()
            .filter(|(p, r)| desired.get(*p) != Some(*r))
            .map(|(p, _)| p.clone())
            .chain(self.uncertain.iter().cloned())
            .collect();
        let add: Vec<_> = desired
            .iter()
            .filter(|(p, r)| self.watched.get(*p) != Some(*r))
            .map(|(p, r)| (p.clone(), *r))
            .collect();
        if remove.is_empty() && add.is_empty() {
            return;
        }
        let mut next = self.watched.clone();
        let mut failed_removals = BTreeSet::new();
        let mut acknowledged = BTreeSet::new();
        let mut paths = self.watcher.as_mut().expect("initialized").paths_mut();
        for path in &remove {
            match paths.remove(path) {
                Ok(()) => {
                    next.remove(path);
                    acknowledged.insert(path.clone());
                }
                Err(err) if matches!(err.kind, notify::ErrorKind::WatchNotFound) => {
                    next.remove(path);
                    acknowledged.insert(path.clone());
                }
                Err(err) => {
                    errors.push(format!("remove {}: {err}", path.display()));
                    failed_removals.insert(path.clone());
                }
            }
        }
        for (path, registration) in &add {
            if failed_removals.contains(path) {
                continue;
            }
            match paths.add(path, registration.mode) {
                Ok(()) => {
                    next.insert(path.clone(), *registration);
                    acknowledged.insert(path.clone());
                }
                Err(err) => errors.push(format!("add {}: {err}", path.display())),
            }
        }
        // shortcut: notify 8.2.0 hides some macOS startup failures; revisit when its API reports them.
        match paths.commit() {
            Ok(()) => {
                self.watched = next;
                for path in acknowledged {
                    self.uncertain.remove(&path);
                }
            }
            Err(err) => {
                errors.push(format!("commit: {err}"));
                self.uncertain.extend(remove);
                self.uncertain.extend(add.into_iter().map(|(path, _)| path));
                // A failed macOS commit can stop the shared stream, including unchanged roots.
                if cfg!(target_os = "macos") {
                    self.uncertain.extend(self.watched.keys().cloned());
                }
                for path in &self.uncertain {
                    self.watched.remove(path);
                }
            }
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
    fn flush(&mut self, raw: BTreeSet<PathBuf>, mut rescan: bool) -> Option<Msg> {
        let mut listings = Listings::new();
        // The roots whose walk keeps `path`, listing its parent `dir`.
        let mut keeping = |s: &Self, dir: &Path, path: &Path| -> Vec<usize> {
            (0..s.roots.len())
                .filter(|&i| s.listed(&mut listings, i, dir, path))
                .collect()
        };
        let mut report = BTreeSet::new();
        for path in raw.iter().filter(|p| self.keep(p)) {
            // An ignored file in a kept directory is left out, unless a buffer visits it.
            let ignored = match path.parent().filter(|dir| self.kept(dir)) {
                Some(dir)
                    if !is_ignore(path)
                        && !self.files.contains(path)
                        && fs::symlink_metadata(path).is_ok() =>
                {
                    keeping(self, dir, path).is_empty()
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
                for i in keeping(self, parent, path) {
                    let root = &mut self.roots[i];
                    // Files made before the watch was in place are reported.
                    for entry in walker(path, &root.path, &root.patterns).build().flatten() {
                        match entry.file_type().is_some_and(|t| t.is_dir()) {
                            true => _ = root.dirs.insert(entry.into_path()),
                            false => _ = report.insert(entry.into_path()),
                        }
                    }
                    changed = true;
                }
            } else if !dir && self.kept(path) && !self.roots.iter().any(|r| r.path == *path) {
                for root in &mut self.roots {
                    root.dirs.retain(|d| !d.starts_with(path));
                }
                changed = true;
            }
        }
        let replaced = raw.iter().any(|path| {
            self.uncertain.contains(path)
                || self
                    .watched
                    .get(path)
                    .is_some_and(|r| identity(path).ok() != Some(r.identity))
        });
        rescan |= raw.iter().any(|path| {
            identity(path).is_ok_and(|id| {
                self.uncertain.contains(path)
                    || self.watched.get(path).is_some_and(|r| r.identity != id)
            })
        });
        if changed || replaced {
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
            roots: roots
                .iter()
                .map(|r| (r.to_path_buf(), Vec::new()))
                .collect(),
            files: files.iter().map(|f| f.to_path_buf()).collect(),
            directories: vec![
                roots
                    .iter()
                    .flat_map(|root| {
                        crate::files::inventory(root, &Patterns::lenient(&[]))
                            .unwrap()
                            .1
                    })
                    .collect::<Vec<_>>()
                    .into(),
            ],
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

    #[derive(Default)]
    struct Fake {
        installed: BTreeMap<PathBuf, RecursiveMode>,
        calls: Vec<(String, PathBuf)>,
        add: BTreeSet<PathBuf>,
        remove: BTreeSet<PathBuf>,
        commit: bool,
    }

    type FakeBackend = std::sync::Arc<std::sync::Mutex<Fake>>;

    struct FakePaths(FakeBackend);

    impl Backend for FakeBackend {
        fn paths_mut(&mut self) -> Box<dyn notify::PathsMut + '_> {
            Box::new(FakePaths(self.clone()))
        }
    }

    impl notify::PathsMut for FakePaths {
        fn add(&mut self, path: &Path, mode: RecursiveMode) -> notify::Result<()> {
            let mut fake = self.0.lock().unwrap();
            fake.calls.push(("add".into(), path.into()));
            if fake.add.contains(path) {
                return Err(notify::Error::generic("add refused"));
            }
            fake.installed.insert(path.into(), mode);
            Ok(())
        }
        fn remove(&mut self, path: &Path) -> notify::Result<()> {
            let mut fake = self.0.lock().unwrap();
            fake.calls.push(("remove".into(), path.into()));
            if fake.remove.contains(path) {
                return Err(notify::Error::generic("remove refused"));
            }
            fake.installed.remove(path);
            Ok(())
        }
        fn commit(self: Box<Self>) -> notify::Result<()> {
            if self.0.lock().unwrap().commit {
                Err(notify::Error::generic("commit uncertain"))
            } else {
                Ok(())
            }
        }
    }

    fn fake_state() -> (State, FakeBackend) {
        let backend = FakeBackend::default();
        let cloned = backend.clone();
        (
            State::with_factory(Box::new(move || Ok(Box::new(cloned.clone())))),
            backend,
        )
    }

    #[test]
    fn replacement_and_equal_scope_recovery_preserve_unaffected_registrations() {
        let a = tree(&[("nested/a", "")]);
        let b = tree(&[("b", "")]);
        let (mut s, backend) = fake_state();
        let initial = set(&[a.path(), b.path()], &[]);
        s.set(initial.clone());
        backend.lock().unwrap().calls.clear();
        s.set(initial.clone());
        assert_eq!(backend.lock().unwrap().calls.len(), 0);
        let old = a.path().join("old");
        fs::rename(a.path().join("nested"), &old).unwrap();
        fs::create_dir(a.path().join("nested")).unwrap();
        s.set(initial);
        assert_eq!(s.watched[b.path()].identity, identity(b.path()).unwrap());
        assert!(
            backend
                .lock()
                .unwrap()
                .calls
                .iter()
                .all(|(_, p)| p != b.path())
        );
        if cfg!(target_os = "linux") {
            assert_eq!(
                s.watched[&a.path().join("nested")].identity,
                identity(&a.path().join("nested")).unwrap()
            );
        }
        s.invalidate(a.path());
        backend.lock().unwrap().calls.clear();
        s.sync();
        assert!(
            backend
                .lock()
                .unwrap()
                .calls
                .iter()
                .any(|(op, p)| op == "add" && p == a.path())
        );
        assert!(s.watched.contains_key(a.path()));
    }

    #[test]
    fn failed_add_remove_and_commit_repair_only_after_acknowledgment() {
        let a = tree(&[("a", "")]);
        let b = tree(&[("b", "")]);
        let (mut s, backend) = fake_state();
        backend.lock().unwrap().add.insert(a.path().into());
        s.set(set(&[a.path(), b.path()], &[]));
        assert_eq!(s.watched.keys().collect::<Vec<_>>(), [b.path()]);
        assert!(s.retry.is_some());
        backend.lock().unwrap().add.clear();
        s.sync();
        assert_eq!(s.watched.len(), 2);
        backend.lock().unwrap().remove.insert(a.path().into());
        s.set(set(&[b.path()], &[]));
        assert_eq!(s.watched.len(), 2);
        backend.lock().unwrap().remove.clear();
        s.sync();
        assert_eq!(s.watched.keys().collect::<Vec<_>>(), [b.path()]);
        backend.lock().unwrap().commit = true;
        s.set(set(&[a.path(), b.path()], &[]));
        assert!(s.uncertain.contains(a.path()));
        assert!(s.retry.is_some());
        backend.lock().unwrap().commit = false;
        s.sync();
        assert_eq!(s.watched.len(), 2);
        assert_eq!(s.uncertain.len(), 0);
        assert_eq!(s.retry, None);
        assert_eq!(s.messages.last().unwrap(), "File watching recovered");
    }

    #[test]
    fn retries_are_capped_report_changes_once_and_cancel_on_release() {
        let dir = tree(&[("a", "")]);
        let (mut s, backend) = fake_state();
        backend.lock().unwrap().add.insert(dir.path().into());
        s.set(set(&[dir.path()], &[]));
        s.messages.clear();
        let now = Instant::now();
        s.attempts = 0;
        for seconds in [1, 2, 4, 8, 16, 30, 30] {
            s.sync_at(now);
            assert_eq!(s.retry, Some(now + Duration::from_secs(seconds)));
        }
        assert_eq!(s.messages.len(), 0);
        backend.lock().unwrap().commit = true;
        s.sync_at(now);
        assert_eq!(s.messages.len(), 1);
        s.set(WatchSet::default());
        assert_eq!(s.retry, None);
        assert_eq!(s.watched.len(), 0);
    }

    #[test]
    fn a_failed_root_keeps_a_confirmed_visited_parent() {
        let dir = tree(&[("child/a", "")]);
        let (mut s, backend) = fake_state();
        backend.lock().unwrap().add.insert(dir.path().into());
        s.set(set(&[dir.path()], &[&dir.path().join("child/a")]));
        assert!(s.watched.contains_key(&dir.path().join("child")));
        assert!(s.retry.is_some());
        backend.lock().unwrap().add.clear();
        s.sync();
        assert!(s.covered(&dir.path().join("child")));
        assert_eq!(s.retry, None);
    }

    #[test]
    fn initialization_and_registration_retry_without_filesystem_events() {
        for initialization in [true, false] {
            let dir = tree(&[("a", "")]);
            let desired = set(&[dir.path()], &[]);
            let backend = FakeBackend::default();
            if !initialization {
                backend.lock().unwrap().add.insert(dir.path().into());
            }
            let worker_backend = backend.clone();
            let (tx, rx) = mpsc::channel();
            let (reports, received) = mpsc::channel();
            let worker = std::thread::spawn(move || {
                let mut first = initialization;
                let factory = move || {
                    if std::mem::take(&mut first) {
                        Err(notify::Error::generic("initialization refused"))
                    } else {
                        Ok(Box::new(worker_backend.clone()) as Box<dyn Backend>)
                    }
                };
                run(State::with_factory(Box::new(factory)), rx, move |m| {
                    reports.send(m).unwrap();
                });
            });
            tx.send(In::Set(desired)).unwrap();
            assert!(
                matches!(received.recv_timeout(Duration::from_secs(2)).unwrap(), Msg::WatchStatus(s) if s.contains("refused"))
            );
            backend.lock().unwrap().add.clear();
            assert_eq!(
                received.recv_timeout(Duration::from_secs(3)).unwrap(),
                Msg::WatchStatus("File watching recovered".into())
            );
            drop(tx);
            worker.join().unwrap();
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
            s.roots[0].dirs.iter().cloned().collect::<Vec<_>>(),
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
    fn project_patterns_leave_files_and_directories_out() {
        let dir = tree(&[
            ("a.log", ""),
            ("b.rs", ""),
            ("src/c.rs", ""),
            ("notes/d/e", ""),
        ]);
        let root = dir.path();
        let ignoring = |ignore: &[&str]| WatchSet {
            roots: vec![(
                root.to_path_buf(),
                ignore.iter().map(|p| p.to_string()).collect(),
            )],
            files: Vec::new(),
            directories: vec![
                crate::files::inventory(
                    root,
                    &Patterns::lenient(&ignore.iter().map(|p| p.to_string()).collect::<Vec<_>>()),
                )
                .unwrap()
                .1
                .into(),
            ],
        };
        let mut s = state();
        s.set(ignoring(&["*.log"]));
        let raw = ["a.log", "b.rs"].map(|n| root.join(n));
        let Some(Msg::FilesChanged { paths, .. }) = s.flush(raw.into(), false) else {
            panic!("no report");
        };
        assert_eq!(paths, [(root.join("b.rs"), Disk::File)]);
        // An allow-list keeps only the root and `notes/` with what is under it.
        s.set(ignoring(&["*", "!notes/"]));
        let kept = [root.to_path_buf(), root.join("notes"), root.join("notes/d")];
        assert_eq!(s.roots.len(), 1, "the old patterns' root is gone");
        assert_eq!(s.roots[0].dirs.iter().cloned().collect::<Vec<_>>(), kept);
        if cfg!(target_os = "linux") {
            assert_eq!(s.watched.keys().cloned().collect::<Vec<_>>(), kept);
        }
        let raw = ["b.rs", "src/c.rs", "notes/d/e"].map(|n| root.join(n));
        let Some(Msg::FilesChanged { paths, .. }) = s.flush(raw.into(), false) else {
            panic!("no report");
        };
        assert_eq!(paths, [(root.join("notes/d/e"), Disk::File)]);
        // A new directory the patterns leave out is neither kept nor scanned.
        fs::create_dir(root.join("new")).unwrap();
        fs::write(root.join("new/f"), "").unwrap();
        assert_eq!(s.flush([root.join("new")].into(), false), None);
        assert!(!s.kept(&root.join("new")));
        // A root shared by projects with different patterns: what either keeps is watched.
        let mut both = ignoring(&["*.log"]);
        let other = ignoring(&["*", "!notes/"]);
        both.roots.extend(other.roots);
        both.directories.extend(other.directories);
        s.set(both);
        assert_eq!(s.roots.len(), 2);
        assert!(s.kept(&root.join("src")) && s.kept(&root.join("notes/d")));
        let raw = ["a.log", "src/c.rs", "notes/d/e"].map(|n| root.join(n));
        let Some(Msg::FilesChanged { paths, .. }) = s.flush(raw.into(), false) else {
            panic!("no report");
        };
        let want = [
            (root.join("notes/d/e"), Disk::File),
            (root.join("src/c.rs"), Disk::File),
        ];
        assert_eq!(paths, want);
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
        assert_eq!(s.roots[0].dirs.iter().collect::<Vec<_>>(), [root]);
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
