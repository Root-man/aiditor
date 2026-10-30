//! Project grep: the files the index lists, searched with ripgrep's crates on the blocking pool.

use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, SyncSender, TryRecvError};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use aiditor_core::{GrepOp, GrepSearch, Hit, Msg};
use grep_matcher::Matcher;
use grep_regex::{RegexMatcher, RegexMatcherBuilder};
use grep_searcher::sinks::Lossy;
use grep_searcher::{BinaryDetection, SearcherBuilder};
use std::sync::mpsc;
use tokio::sync::mpsc as tokio_mpsc;

/// How long hits wait to be sent, so a fast search doesn't flood the core with messages.
const BATCH: Duration = Duration::from_millis(50);

/// Runs searches one at a time: each one, or a cancel, stops the one before.
#[derive(Default)]
pub struct Grep {
    /// The running search's number; 0 when cancelled.
    latest: Arc<AtomicU64>,
}

impl Grep {
    /// Carries out `op` on Tokio's blocking pool, sending the search's messages on `tx`.
    pub fn run(&self, op: GrepOp, tx: tokio_mpsc::Sender<Msg>) {
        let GrepOp::Search(op) = op else {
            return self.latest.store(0, Ordering::Relaxed);
        };
        // Stored here rather than in the task, so the newest search wins whichever task starts first.
        self.latest.store(op.id, Ordering::Relaxed);
        let latest = self.latest.clone();
        tokio::task::spawn_blocking(move || {
            // A closed channel means the app is gone; nobody reads the rest.
            let cancelled = || latest.load(Ordering::Relaxed) != op.id || tx.is_closed();
            search(&op, cancelled, |msg| _ = tx.blocking_send(msg));
        });
    }
}

/// Most files walked but not yet delivered, per worker.
const WINDOW: usize = 4;
/// Most hits a file holds back while earlier files are delivered.
const HELD: usize = 64;

/// A file for a worker: its root's index, its path and where its hits go.
type Job = (usize, PathBuf, SyncSender<Hit>);

/// Searches `op`'s roots on `op.workers` threads, sending `Msg::Grepped` batches in root, file
/// and line order until done or `op.limit` hits. `cancelled` is checked before each file, after
/// each match and before each send; once it is true nothing more is sent.
pub fn search(op: &GrepSearch, cancelled: impl Fn() -> bool + Sync, send: impl FnMut(Msg)) {
    search_with_clock(op, cancelled, send, Instant::now);
}

fn search_with_clock(
    op: &GrepSearch,
    cancelled: impl Fn() -> bool + Sync,
    mut send: impl FnMut(Msg),
    now: impl Fn() -> Instant,
) {
    let mut reply = |hits, done| {
        if cancelled() {
            return;
        }
        send(Msg::Grepped {
            search: op.id,
            hits,
            done,
        })
    };
    let matcher = match RegexMatcherBuilder::new()
        .case_smart(true)
        .build(&op.pattern)
    {
        Ok(matcher) => matcher,
        Err(err) => return reply(Err(err.to_string()), true),
    };
    // As if a batch went out a deadline ago, so the first hit is sent at once; one `now()` call,
    // since a test clock advances on each.
    let t = now();
    let (mut hits, mut found, mut sent) = (Vec::new(), 0, t.checked_sub(BATCH).unwrap_or(t));
    let mut flush = |hits: &mut Vec<Hit>, done| {
        if cancelled() {
            return false;
        }
        let at = now();
        if done || (!hits.is_empty() && at.duration_since(sent) >= BATCH) {
            reply(Ok(std::mem::take(hits)), done);
            sent = at;
        }
        true
    };
    let stop = AtomicBool::new(false);
    let halted = || stop.load(Ordering::Relaxed) || cancelled();
    let (jobs, queue) = mpsc::channel::<Job>();
    let queue = Mutex::new(queue);
    std::thread::scope(|scope| {
        for _ in 0..op.workers.get() {
            scope.spawn(|| worker(op, &matcher, &queue, &halted));
        }
        // Each outstanding file's hits, in walk order; only the front one is delivered.
        let mut pending: VecDeque<Receiver<Hit>> = VecDeque::new();
        // Delivers the front files' hits, waiting for them while more than `keep` are
        // outstanding; false once the search must stop.
        let mut drain = |pending: &mut VecDeque<Receiver<Hit>>, keep: usize| {
            while let Some(front) = pending.front() {
                let hit = if pending.len() > keep {
                    front.recv().ok()
                } else {
                    match front.try_recv() {
                        Err(TryRecvError::Empty) => break,
                        hit => hit.ok(),
                    }
                };
                let Some(hit) = hit else {
                    pending.pop_front();
                    if !flush(&mut hits, false) {
                        return false;
                    }
                    continue;
                };
                hits.push(hit);
                found += 1;
                if !flush(&mut hits, false) || found >= op.limit {
                    return false;
                }
            }
            true
        };
        let patterns = crate::files::Patterns::lenient(&op.ignore);
        let files = (op.roots.iter().enumerate())
            .flat_map(|(i, root)| crate::files::walk(root, &patterns).map(move |f| (i, f)));
        let window = WINDOW * op.workers.get();
        let mut going = op.limit > 0;
        for (root, file) in files {
            if !going || cancelled() {
                break;
            }
            let (tx, rx) = mpsc::sync_channel(HELD);
            _ = jobs.send((root, file, tx));
            pending.push_back(rx);
            going = drain(&mut pending, window - 1);
        }
        going = going && drain(&mut pending, 0);
        // Stops the workers, failing their sends to the dropped receivers.
        stop.store(true, Ordering::Relaxed);
        drop((jobs, pending));
        if going || found >= op.limit {
            flush(&mut hits, true);
        }
    });
}

/// Searches the files `queue` hands out with its own searcher, until it closes or `halted`.
fn worker(
    op: &GrepSearch,
    matcher: &RegexMatcher,
    queue: &Mutex<Receiver<Job>>,
    halted: &(impl Fn() -> bool + Sync),
) {
    let mut searcher = SearcherBuilder::new()
        .line_number(true)
        .binary_detection(BinaryDetection::quit(b'\0'))
        .build();
    loop {
        // Bound first, so the lock is released before searching.
        let job = queue.lock().unwrap().recv();
        let Ok((root, file, tx)) = job else { return };
        if halted() {
            return;
        }
        let root = &op.roots[root];
        let path = file.strip_prefix(root).unwrap_or(&file);
        let mut found = 0;
        let sink = Lossy(|line, text: &str| {
            if halted() {
                return Ok(false);
            }
            let text = text.strip_suffix('\n').unwrap_or(text);
            let text = text.strip_suffix('\r').unwrap_or(text);
            // shortcut: a non-UTF-8 line's lossy text may not match, giving column 0; match bytes if that matters.
            let start = (matcher.find(text.as_bytes()).ok().flatten()).map_or(0, |m| m.start());
            let hit = Hit {
                root: root.clone(),
                path: path.to_path_buf(),
                line: line as usize - 1,
                col: text[..start].chars().count(),
                text: text.to_string(),
            };
            found += 1;
            // A failed send means the coordinator stopped.
            Ok(tx.send(hit).is_ok() && found < op.limit)
        });
        // Unreadable files are skipped, as `rg` skips them.
        _ = searcher.search_path(matcher, &file, sink);
    }
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;
    use std::fs;
    use std::num::NonZeroUsize;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::AtomicUsize;

    use super::*;

    fn tree(files: &[(&str, &str)]) -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        for (name, text) in files {
            let path = dir.path().join(name);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, text).unwrap();
        }
        dir
    }

    fn op(roots: &[&Path], pattern: &str, limit: usize) -> GrepSearch {
        GrepSearch {
            id: 7,
            roots: roots.iter().map(|r| r.to_path_buf()).collect(),
            ignore: Vec::new(),
            pattern: pattern.into(),
            limit,
            workers: NonZeroUsize::MIN,
        }
    }

    fn with_workers(op: GrepSearch, workers: usize) -> GrepSearch {
        GrepSearch {
            workers: NonZeroUsize::new(workers).unwrap(),
            ..op
        }
    }

    /// Every message of an uncancelled search.
    fn run(op: &GrepSearch) -> Vec<Msg> {
        let mut msgs = Vec::new();
        search(op, || false, |msg| msgs.push(msg));
        msgs
    }

    /// The hits of an uncancelled search as `path:line:col:text`, checking it ends done.
    fn hits(op: &GrepSearch) -> Vec<String> {
        let msgs = run(op);
        assert!(matches!(msgs.last(), Some(Msg::Grepped { done: true, .. })));
        let mut found = Vec::new();
        for msg in msgs {
            let Msg::Grepped { search, hits, .. } = msg else {
                panic!("not grepped");
            };
            assert_eq!(search, 7);
            for h in hits.unwrap() {
                let path = h.path.display();
                found.push(format!("{path}:{}:{}:{}", h.line, h.col, h.text));
            }
        }
        found
    }

    #[test]
    fn skips_ignored_and_binary_files() {
        let dir = tree(&[
            (".gitignore", "target/\n"),
            ("target/out", "needle\n"),
            ("src/a.rs", "needle\n"),
            ("blob", "needle\0\n"),
        ]);
        assert_eq!(
            hits(&op(&[dir.path()], "needle", 100)),
            ["src/a.rs:0:0:needle"]
        );
    }

    #[test]
    fn skips_what_the_project_patterns_leave_out() {
        let dir = tree(&[("notes/n.md", "needle\n"), ("src/a.rs", "needle\n")]);
        let op = GrepSearch {
            ignore: vec!["*".into(), "!notes/".into()],
            ..op(&[dir.path()], "needle", 100)
        };
        assert_eq!(hits(&op), ["notes/n.md:0:0:needle"]);
    }

    #[test]
    fn lists_roots_in_order_and_files_in_path_order() {
        let (app, lib) = (
            tree(&[("b", "x"), ("a/z", "x"), ("a.rs", "x")]),
            tree(&[("a", "x")]),
        );
        let found = hits(&op(&[lib.path(), app.path()], "x", 100));
        assert_eq!(found, ["a:0:0:x", "a/z:0:0:x", "a.rs:0:0:x", "b:0:0:x"]);
        let msgs = run(&op(&[lib.path(), app.path()], "x", 100));
        let hits = msgs.into_iter().flat_map(|m| match m {
            Msg::Grepped { hits, .. } => hits.unwrap(),
            _ => panic!("not grepped"),
        });
        let roots: Vec<PathBuf> = hits.map(|h| h.root).collect();
        assert_eq!(roots, [lib.path(), app.path(), app.path(), app.path()]);
    }

    #[test]
    fn ignores_case_until_the_pattern_has_an_upper_case_letter() {
        let dir = tree(&[("f", "Foo\nfoo\n")]);
        assert_eq!(
            hits(&op(&[dir.path()], "foo", 100)),
            ["f:0:0:Foo", "f:1:0:foo"]
        );
        assert_eq!(hits(&op(&[dir.path()], "Foo", 100)), ["f:0:0:Foo"]);
    }

    #[test]
    fn ends_an_invalid_pattern_with_its_error() {
        let dir = tree(&[("f", "(\n")]);
        let [Msg::Grepped { hits, done, .. }] = &run(&op(&[dir.path()], "(", 100))[..] else {
            panic!("not one message");
        };
        assert!(*done);
        // The core echoes the last line.
        let err = hits.as_ref().unwrap_err();
        assert_eq!(err.lines().last(), Some("error: unclosed group"), "{err}");
    }

    #[test]
    fn a_hit_is_one_line_with_its_first_match_column_in_chars() {
        let dir = tree(&[("src/a.rs", "fn f() {\r\nlet été = needle + needle;\r\n")]);
        assert_eq!(
            hits(&op(&[dir.path()], "needle", 100)),
            ["src/a.rs:1:10:let été = needle + needle;"]
        );
    }

    #[test]
    fn stops_at_the_limit() {
        let dir = tree(&[("a", "x\nx\nx\nx\nx\nx\n"), ("b", "x\nx\nx\nx\n")]);
        assert_eq!(hits(&op(&[dir.path()], "x", 3)).len(), 3);
        assert_eq!(hits(&op(&[dir.path()], "x", 8)).len(), 8);
        assert_eq!(hits(&op(&[dir.path()], "x", 0)).len(), 0);
    }

    #[test]
    fn the_production_search_fills_the_prompt_to_its_exact_ten_thousand_hit_cap() {
        let dir = tree(&[("a", &"needle\n".repeat(10_005))]);
        let mut core = aiditor_core::Core::default();
        core.launch(dir.path(), dir.path());
        let now = Instant::now();
        for key in aiditor_core::key::parse_keys("C-x p g needle").unwrap() {
            core.handle(now, Msg::Key(key));
        }
        let effects = core.handle(now + Duration::from_secs(1), Msg::Tick);
        let op = effects
            .into_iter()
            .find_map(|e| match e {
                aiditor_core::Effect::Grep(GrepOp::Search(op)) => Some(op),
                _ => None,
            })
            .unwrap();
        assert_eq!(op.limit, 10_000);
        let mut total = 0;
        let mut done = 0;
        search(
            &op,
            || false,
            |msg| {
                if let Msg::Grepped {
                    hits,
                    done: finished,
                    ..
                } = &msg
                {
                    total += hits.as_ref().unwrap().len();
                    done += usize::from(*finished);
                }
                core.handle(now + Duration::from_secs(1), msg);
            },
        );
        assert_eq!((total, done), (10_000, 1));
        let prompt = core.frame().minibuffer.unwrap();
        assert_eq!(prompt.count, "1/10000 ");
        assert!(prompt.candidates[0].ends_with("a:1:needle"));
    }

    /// Each message as its hit count and whether it is done.
    fn batches(msgs: &[Msg]) -> Vec<(usize, bool)> {
        (msgs.iter())
            .map(|m| match m {
                Msg::Grepped { hits, done, .. } => (hits.as_ref().unwrap().len(), *done),
                _ => panic!("not grepped"),
            })
            .collect()
    }

    #[test]
    fn one_ongoing_file_streams_ordered_batches_on_the_deadline() {
        let dir = tree(&[("a", "été x\nété x\nété x\nété x\n")]);
        let start = Instant::now();
        let elapsed = Cell::new(Duration::ZERO);
        let now = || {
            let at = start + elapsed.get();
            elapsed.set(elapsed.get() + BATCH / 2);
            at
        };
        let mut msgs = Vec::new();
        search_with_clock(
            &op(&[dir.path()], "x", 100),
            || false,
            |m| msgs.push(m),
            now,
        );
        assert_eq!(
            batches(&msgs),
            [(1, false), (2, false), (1, false), (0, true)]
        );
        let positions: Vec<_> = msgs
            .into_iter()
            .flat_map(|m| match m {
                Msg::Grepped { search, hits, .. } => {
                    assert_eq!(search, 7);
                    hits.unwrap()
                        .into_iter()
                        .map(|h| (h.line, h.col))
                        .collect::<Vec<_>>()
                }
                _ => panic!("not grepped"),
            })
            .collect();
        assert_eq!(positions, [(0, 4), (1, 4), (2, 4), (3, 4)]);
    }

    #[test]
    fn a_search_cancelled_after_an_in_file_batch_sends_nothing_more() {
        let dir = tree(&[("a", &"x\n".repeat(100))]);
        let sent = AtomicBool::new(false);
        let ticks = Cell::new(0);
        let start = Instant::now();
        let now = || {
            let at = start + BATCH * ticks.get();
            ticks.set(ticks.get() + 1);
            at
        };
        let mut msgs = Vec::new();
        search_with_clock(
            &op(&[dir.path()], "x", 100),
            || sent.load(Ordering::Relaxed),
            |m| {
                sent.store(true, Ordering::Relaxed);
                msgs.push(m);
            },
            now,
        );
        assert_eq!(batches(&msgs), [(1, false)]);
    }

    #[test]
    fn a_cancelled_search_stops_at_once_and_sends_nothing() {
        let names: Vec<String> = (0..200).map(|i| format!("f{i:03}")).collect();
        let files: Vec<(&str, &str)> = names.iter().map(|n| (n.as_str(), "x\n")).collect();
        let (many, big) = (tree(&files), tree(&[("big", &"x\n".repeat(10_000))]));
        // Four workers fill their queues and held hits before the cancel.
        for (root, workers) in [
            (many.path(), 1),
            (big.path(), 1),
            (many.path(), 4),
            (big.path(), 4),
        ] {
            // Cancelled from the 11th check on, wherever the search is by then.
            let (checks, after) = (AtomicUsize::new(0), AtomicUsize::new(0));
            let cancelled = || {
                let cancelled = checks.fetch_add(1, Ordering::Relaxed) >= 10;
                after.fetch_add(usize::from(cancelled), Ordering::Relaxed);
                cancelled
            };
            let mut msgs = Vec::new();
            let op = with_workers(op(&[root], "x", 1 << 20), workers);
            search(&op, cancelled, |m| {
                assert_eq!(after.load(Ordering::Relaxed), 0, "sent after the cancel");
                msgs.push(m);
            });
            let done = msgs
                .iter()
                .any(|m| matches!(m, Msg::Grepped { done: true, .. }));
            // Each worker checks once in its file and once before its next; the coordinator twice.
            let after = after.load(Ordering::Relaxed);
            assert!(!done && after <= 2 + 2 * workers, "{after}");
        }
    }

    /// Every hit of a search with `workers`, as `root/path:line:col:text`, in order.
    fn ordered(roots: &[&Path], limit: usize, workers: usize) -> Vec<String> {
        let msgs = run(&with_workers(op(roots, "x", limit), workers));
        assert_eq!(batches(&msgs).iter().filter(|(_, done)| *done).count(), 1);
        assert!(matches!(msgs.last(), Some(Msg::Grepped { done: true, .. })));
        (msgs.into_iter())
            .flat_map(|m| match m {
                Msg::Grepped { hits, .. } => hits.unwrap(),
                _ => panic!("not grepped"),
            })
            .map(|h| {
                let (root, path) = (h.root.display(), h.path.display());
                format!("{root}/{path}:{}:{}:{}", h.line, h.col, h.text)
            })
            .collect()
    }

    #[test]
    fn workers_find_the_same_hits_in_the_same_order() {
        let names: Vec<String> = (0..60).map(|i| format!("d{}/f{i:02}", i % 7)).collect();
        let texts: Vec<String> = (0..60)
            .map(|i| "été x\nno\n".repeat(i % 5) + &"x\n".repeat(if i == 9 { 500 } else { 0 }))
            .collect();
        let files: Vec<(&str, &str)> = (names.iter().zip(&texts))
            .map(|(n, t)| (n.as_str(), t.as_str()))
            .chain([("blob", "x\0\n"), (".gitignore", "d6/\n"), ("Big", "X\n")])
            .collect();
        let (a, b) = (tree(&files), tree(&files));
        let roots = [a.path(), b.path()];
        let one = ordered(&roots, 1 << 20, 1);
        // Per root: 120 lines in the small files, less 14 under d6/, 500 in d2/f09 and `Big`.
        assert_eq!(one.len(), 2 * 607);
        for workers in [2, 3, 8, 200] {
            assert_eq!(ordered(&roots, 1 << 20, workers), one, "{workers} workers");
        }
        // The limit cuts the same ordered hits, even inside a file held back for later.
        for limit in [0, 3, 40, 600] {
            assert_eq!(ordered(&roots, limit, 4), one[..limit], "limit {limit}");
        }
    }

    #[test]
    fn files_are_searched_on_at_most_the_given_workers_at_once() {
        let names: Vec<String> = (0..40).map(|i| format!("f{i:02}")).collect();
        let files: Vec<(&str, &str)> = names.iter().map(|n| (n.as_str(), "x\n")).collect();
        let dir = tree(&files);
        let coordinator = std::thread::current().id();
        let (inside, most) = (AtomicUsize::new(0), AtomicUsize::new(0));
        // Workers check before each file and at each match; holding them there makes them overlap.
        let cancelled = || {
            if std::thread::current().id() != coordinator {
                let now = inside.fetch_add(1, Ordering::SeqCst) + 1;
                most.fetch_max(now, Ordering::SeqCst);
                std::thread::sleep(Duration::from_millis(2));
                inside.fetch_sub(1, Ordering::SeqCst);
            }
            false
        };
        let op = with_workers(op(&[dir.path()], "x", 100), 3);
        let mut found = 0;
        search(&op, cancelled, |m| {
            if let Msg::Grepped { hits, .. } = m {
                found += hits.unwrap().len();
            }
        });
        assert_eq!(found, 40);
        let most = most.load(Ordering::SeqCst);
        assert!((2..=3).contains(&most), "{most} at once");
    }

    #[test]
    fn a_later_file_finishing_first_still_streams_in_order() {
        let dir = tree(&[("a", &"x\n".repeat(2_000)), ("b", "x\n")]);
        let coordinator = std::thread::current().id();
        let calls = Mutex::new(std::collections::HashMap::new());
        // `b`'s worker checks twice (its file, its match); `a`'s waits at its third until then.
        let cancelled = || {
            let me = std::thread::current().id();
            if me == coordinator {
                return false;
            }
            let mine = {
                let mut calls = calls.lock().unwrap();
                *calls.entry(me).and_modify(|n| *n += 1).or_insert(1)
            };
            if mine == 3 {
                while !(calls.lock().unwrap().iter()).any(|(&t, &n)| t != me && n >= 2) {
                    std::thread::sleep(Duration::from_millis(1));
                }
                // Lets `b`'s worker send its hit and close its file.
                std::thread::sleep(Duration::from_millis(20));
            }
            false
        };
        let start = Instant::now();
        let elapsed = Cell::new(Duration::ZERO);
        let now = || {
            let at = start + elapsed.get();
            elapsed.set(elapsed.get() + BATCH / 2);
            at
        };
        let mut msgs = Vec::new();
        let op = with_workers(op(&[dir.path()], "x", 1 << 20), 2);
        search_with_clock(&op, cancelled, |m| msgs.push(m), now);
        let batches = batches(&msgs);
        // The first hit at once, then a batch every other hit, and one done.
        assert_eq!(batches[0], (1, false));
        assert!(batches.len() > 100, "{}", batches.len());
        assert_eq!(batches.iter().filter(|(_, done)| *done).count(), 1);
        let paths: Vec<_> = (msgs.into_iter())
            .flat_map(|m| match m {
                Msg::Grepped { hits, .. } => hits.unwrap(),
                _ => panic!("not grepped"),
            })
            .map(|h| (h.path.display().to_string(), h.line))
            .collect();
        let expected: Vec<_> = (0..2_000)
            .map(|l| ("a".into(), l))
            .chain([("b".into(), 0)])
            .collect();
        assert_eq!(paths, expected);
    }

    #[test]
    fn a_cancel_while_held_hits_are_full_stops_the_workers() {
        let names: Vec<String> = (0..40).map(|i| format!("f{i:02}")).collect();
        let body = "x\n".repeat(HELD * 3);
        let files: Vec<(&str, &str)> = names.iter().map(|n| (n.as_str(), body.as_str())).collect();
        let dir = tree(&files);
        let cancel = AtomicBool::new(false);
        let mut sends = 0;
        let op = with_workers(op(&[dir.path()], "x", 1 << 20), 4);
        search(
            &op,
            || cancel.load(Ordering::Relaxed),
            |_| {
                sends += 1;
                // Holding the first batch lets every worker fill its file's held hits and block.
                std::thread::sleep(Duration::from_millis(100));
                cancel.store(true, Ordering::Relaxed);
            },
        );
        // Returning at all means the blocked workers were released.
        assert_eq!(sends, 1);
    }

    #[test]
    fn a_newer_search_or_a_cancel_supersedes_queued_searches() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .max_blocking_threads(1)
            .build()
            .unwrap();
        let _guard = runtime.enter();
        let hold = || {
            let (started, ready) = std::sync::mpsc::channel();
            let (release, held) = std::sync::mpsc::channel::<()>();
            runtime.spawn_blocking(move || {
                started.send(()).unwrap();
                _ = held.recv();
            });
            ready.recv_timeout(Duration::from_secs(5)).unwrap();
            release
        };
        let dir = tree(&[("a", "x\n")]);
        let (tx, mut rx) = crate::channel();
        let grep = Grep::default();
        let search = |id, root: &Path| {
            GrepOp::Search(GrepSearch {
                id,
                ..op(&[root], "x", 1 << 20)
            })
        };
        let mut done = Vec::new();
        let mut take = |msg| {
            if let Msg::Grepped {
                search, done: true, ..
            } = msg
            {
                done.push(search);
            }
        };
        let release = hold();
        grep.run(search(1, dir.path()), tx.clone());
        grep.run(search(2, dir.path()), tx.clone());
        drop(release);
        runtime.block_on(async { while !done_with(&mut rx, &mut take, 2).await {} });
        let release = hold();
        grep.run(search(3, dir.path()), tx.clone());
        grep.run(GrepOp::Cancel, tx);
        drop(release);
        // Ends once every search's task has dropped its sender.
        runtime.block_on(async {
            while let Some(msg) = rx.recv().await {
                take(msg);
            }
        });
        assert_eq!(done, [2]);
    }

    /// Takes the next message, telling whether it is search `id`'s last.
    async fn done_with(
        rx: &mut tokio_mpsc::Receiver<Msg>,
        take: &mut impl FnMut(Msg),
        id: u64,
    ) -> bool {
        let msg = rx.recv().await.unwrap();
        let last = matches!(msg, Msg::Grepped { search, done: true, .. } if search == id);
        take(msg);
        last
    }
}
