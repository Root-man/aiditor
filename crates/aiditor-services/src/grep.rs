//! Project grep: the files the index lists, searched with ripgrep's crates on the blocking pool.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, Instant};

use aiditor_core::{GrepOp, GrepSearch, Hit, Msg};
use grep_matcher::Matcher;
use grep_regex::{RegexMatcher, RegexMatcherBuilder};
use grep_searcher::sinks::Lossy;
use grep_searcher::{BinaryDetection, Searcher, SearcherBuilder};
use ignore::WalkState;
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

const HELD: usize = 64;

/// Searches all roots concurrently and delivers hits in collector arrival order.
pub fn search(op: &GrepSearch, cancelled: impl Fn() -> bool + Sync, send: impl FnMut(Msg)) {
    search_with_hook(op, cancelled, send, |_, _| {});
}

fn search_with_hook(
    op: &GrepSearch,
    cancelled: impl Fn() -> bool + Sync,
    mut send: impl FnMut(Msg),
    hook: impl Fn(&std::path::Path, usize) + Sync,
) {
    let mut reply = |hits, done| {
        if !cancelled() {
            send(Msg::Grepped {
                search: op.id,
                hits,
                done,
            });
        }
    };
    let matcher = match RegexMatcherBuilder::new()
        .case_smart(true)
        .build(&op.pattern)
    {
        Ok(matcher) => matcher,
        Err(err) => return reply(Err(err.to_string()), true),
    };
    if op.limit == 0 || op.roots.is_empty() {
        return reply(Ok(Vec::new()), true);
    }
    let patterns = crate::files::Patterns::lenient(&op.ignore);
    let walk = crate::files::grep_walker(&op.roots, &patterns, op.workers.get()).build_parallel();
    let stop = AtomicBool::new(false);
    let halted = || stop.load(Ordering::Relaxed) || cancelled();
    let (tx, rx) = mpsc::sync_channel::<Hit>(HELD);
    let (matcher, halted, hook) = (&matcher, &halted, &hook);
    std::thread::scope(|scope| {
        scope.spawn(move || {
            walk.run(|| {
                let tx = tx.clone();
                let mut searcher = SearcherBuilder::new()
                    .line_number(true)
                    .binary_detection(BinaryDetection::quit(b'\0'))
                    .build();
                Box::new(move |entry| {
                    if halted() {
                        return WalkState::Quit;
                    }
                    let Ok(entry) = entry else {
                        return WalkState::Continue;
                    };
                    if crate::files::searchable_file(&entry) {
                        hook(entry.path(), 0);
                        search_file(&mut searcher, matcher, op, &entry, &tx, halted, hook);
                    }
                    if halted() {
                        WalkState::Quit
                    } else {
                        WalkState::Continue
                    }
                })
            });
        });
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let (mut hits, mut found, mut deadline) = (Vec::new(), 0, None);
            loop {
                if cancelled() {
                    break;
                }
                let wait = deadline.map_or(BATCH, |at: Instant| {
                    at.saturating_duration_since(Instant::now())
                });
                match rx.recv_timeout(wait) {
                    Ok(hit) => {
                        hits.push(hit);
                        found += 1;
                        if found == op.limit {
                            reply(Ok(std::mem::take(&mut hits)), true);
                            break;
                        }
                        if deadline.is_none() || deadline.is_some_and(|at| Instant::now() >= at) {
                            reply(Ok(std::mem::take(&mut hits)), false);
                            deadline = Some(Instant::now() + BATCH);
                        }
                    }
                    Err(mpsc::RecvTimeoutError::Timeout) => {
                        if !hits.is_empty() {
                            reply(Ok(std::mem::take(&mut hits)), false);
                            deadline = Some(Instant::now() + BATCH);
                        } else {
                            deadline = None;
                        }
                    }
                    Err(mpsc::RecvTimeoutError::Disconnected) => {
                        reply(Ok(hits), true);
                        break;
                    }
                }
            }
        }));
        stop.store(true, Ordering::Relaxed);
        drop(rx);
        if let Err(panic) = result {
            std::panic::resume_unwind(panic);
        }
    });
}

fn search_file(
    searcher: &mut Searcher,
    matcher: &RegexMatcher,
    op: &GrepSearch,
    entry: &ignore::DirEntry,
    tx: &mpsc::SyncSender<Hit>,
    halted: &(impl Fn() -> bool + Sync),
    hook: &(impl Fn(&std::path::Path, usize) + Sync),
) {
    let file = entry.path();
    let Some(root) = op.roots.iter().find(|r| file.starts_with(r)) else {
        return;
    };
    let path = file.strip_prefix(root).unwrap_or(file);
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
            root: root.to_path_buf(),
            path: path.to_path_buf(),
            line: line as usize - 1,
            col: text[..start].chars().count(),
            text: text.to_string(),
        };
        found += 1;
        // A failed send means the coordinator stopped.
        if tx.send(hit).is_err() {
            return Ok(false);
        }
        hook(file, found);
        Ok(found < op.limit && !halted())
    });
    // Unreadable files are skipped, as `rg` skips them.
    _ = searcher.search_path(matcher, file, sink);
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::num::NonZeroUsize;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::AtomicUsize;
    use std::sync::mpsc;

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
    fn lists_all_roots() {
        let (app, lib) = (
            tree(&[("b", "x"), ("a/z", "x"), ("a.rs", "x")]),
            tree(&[("a", "x")]),
        );
        let found = hits(&op(&[lib.path(), app.path()], "x", 100));
        assert_eq!(found.len(), 4);
        let msgs = run(&op(&[lib.path(), app.path()], "x", 100));
        let hits = msgs.into_iter().flat_map(|m| match m {
            Msg::Grepped { hits, .. } => hits.unwrap(),
            _ => panic!("not grepped"),
        });
        let roots: Vec<PathBuf> = hits.map(|h| h.root).collect();
        assert_eq!(
            roots.iter().filter(|r| r.as_path() == lib.path()).count(),
            1
        );
        assert_eq!(
            roots.iter().filter(|r| r.as_path() == app.path()).count(),
            3
        );
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
        for limit in [0, 1, 3] {
            let msgs = run(&with_workers(op(&[dir.path()], "x", limit), 4));
            assert_eq!(batches(&msgs).iter().map(|(n, _)| n).sum::<usize>(), limit);
            assert_eq!(batches(&msgs).iter().filter(|(_, done)| *done).count(), 1);
        }
        assert_eq!(hits(&op(&[dir.path()], "x", 8)).len(), 8);
    }

    #[test]
    fn empty_missing_and_match_free_roots_complete_once() {
        let empty = tempfile::tempdir().unwrap();
        let no_match = tree(&[("a", "ordinary\n")]);
        let missing = empty.path().join("missing");
        for roots in [
            vec![],
            vec![empty.path()],
            vec![no_match.path()],
            vec![&missing],
        ] {
            let msgs = run(&op(&roots, "needle", 10));
            assert_eq!(batches(&msgs), [(0, true)]);
        }
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
        let mut first_seen = false;
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
                if !first_seen && total > 0 {
                    let prompt = core.frame().minibuffer.unwrap();
                    assert_eq!(prompt.count, "   1/1 ");
                    assert!(prompt.prompt.starts_with("[searching]"));
                    first_seen = true;
                }
            },
        );
        assert_eq!((total, done, first_seen), (10_000, 1, true));
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
    fn workers_find_the_same_complete_set_and_each_file_stays_in_line_order() {
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
        let collect = |workers| {
            let msgs = run(&with_workers(op(&roots, "x", 1 << 20), workers));
            assert_eq!(batches(&msgs).iter().filter(|(_, done)| *done).count(), 1);
            let mut lines = std::collections::HashMap::<(PathBuf, PathBuf), usize>::new();
            let mut found = Vec::new();
            for hit in msgs.into_iter().flat_map(|m| match m {
                Msg::Grepped { hits, .. } => hits.unwrap(),
                _ => unreachable!(),
            }) {
                let key = (hit.root.clone(), hit.path.clone());
                if let Some(previous) = lines.insert(key, hit.line) {
                    assert!(previous < hit.line);
                }
                found.push((hit.root, hit.path, hit.line, hit.col, hit.text));
            }
            found.sort();
            found
        };
        let one = collect(1);
        assert_eq!(one.len(), 2 * 607);
        for workers in [2, 3, 8] {
            assert_eq!(collect(workers), one);
        }
    }

    #[test]
    fn multi_root_coverage_matches_the_index() {
        let parent = tempfile::tempdir().unwrap();
        let roots: Vec<_> = ["app", "lib"].map(|name| parent.path().join(name)).into();
        for root in &roots {
            for (name, text) in [
                ("notes/a.md", "needle\n"),
                ("notes/.hidden", "needle\n"),
                ("notes/nested/keep", "needle\n"),
                ("notes/nested/drop", "needle\n"),
                ("src/a.rs", "needle\n"),
                (".git/config", "needle\n"),
                ("notes/nested/.ignore", "drop\n"),
            ] {
                let path = root.join(name);
                fs::create_dir_all(path.parent().unwrap()).unwrap();
                fs::write(path, text).unwrap();
            }
            std::os::unix::fs::symlink(root.join("notes/a.md"), root.join("notes/link.md"))
                .unwrap();
            std::os::unix::fs::symlink(root.join("notes/nested"), root.join("notes/dir-link"))
                .unwrap();
        }
        let ignore = vec!["*".into(), "!notes/".into()];
        let patterns = crate::files::Patterns::new(&ignore).unwrap();
        let expected: std::collections::BTreeSet<_> = roots
            .iter()
            .flat_map(|root| {
                crate::files::index(root, &patterns)
                    .unwrap()
                    .into_iter()
                    .filter_map(|rel| {
                        fs::read(root.join(&rel))
                            .ok()
                            .filter(|body| body.windows(6).any(|part| part == b"needle"))
                            .map(|_| (root.clone(), rel))
                    })
                    .collect::<Vec<_>>()
            })
            .collect();
        let mut search = with_workers(op(&[&roots[0], &roots[1]], "needle", 100), 4);
        search.ignore = ignore;
        let actual: std::collections::BTreeSet<_> = run(&search)
            .into_iter()
            .flat_map(|m| match m {
                Msg::Grepped { hits, .. } => hits.unwrap(),
                _ => unreachable!(),
            })
            .map(|h| (h.root, h.path))
            .collect();
        assert_eq!(actual, expected);
        assert_eq!(actual.len(), 8);
    }

    #[test]
    fn files_are_searched_on_at_most_the_given_workers_at_once() {
        let names: Vec<String> = (0..40).map(|i| format!("f{i:02}")).collect();
        let files: Vec<(&str, &str)> = names.iter().map(|n| (n.as_str(), "x\n")).collect();
        let dir = tree(&files);
        let (inside, most) = (AtomicUsize::new(0), AtomicUsize::new(0));
        let op = with_workers(op(&[dir.path()], "x", 100), 3);
        let mut found = 0;
        search_with_hook(
            &op,
            || false,
            |m| {
                if let Msg::Grepped { hits, .. } = m {
                    found += hits.unwrap().len();
                }
            },
            |_, line| {
                if line == 0 {
                    let now = inside.fetch_add(1, Ordering::SeqCst) + 1;
                    most.fetch_max(now, Ordering::SeqCst);
                    std::thread::sleep(Duration::from_millis(2));
                    inside.fetch_sub(1, Ordering::SeqCst);
                }
            },
        );
        assert_eq!(found, 40);
        assert!((2..=3).contains(&most.load(Ordering::SeqCst)));
    }

    #[test]
    fn later_root_hit_arrives_before_an_earlier_file_finishes() {
        let (early, late) = (tree(&[("slow", "x\n")]), tree(&[("fast", "x\n")]));
        let early_path = early.path().join("slow");
        let search_op = with_workers(op(&[early.path(), late.path()], "x", 100), 2);
        let released = Arc::new(AtomicBool::new(false));
        let release = released.clone();
        let (started_tx, started_rx) = mpsc::channel();
        let (msgs_tx, msgs_rx) = mpsc::channel();
        let handle = std::thread::spawn(move || {
            search_with_hook(
                &search_op,
                || false,
                |m| {
                    msgs_tx.send(m).unwrap();
                },
                |path, line| {
                    if line == 0 && path == early_path {
                        started_tx.send(()).unwrap();
                        while !release.load(Ordering::Relaxed) {
                            std::thread::yield_now();
                        }
                    }
                },
            );
        });
        started_rx.recv_timeout(Duration::from_secs(5)).unwrap();
        let first = msgs_rx.recv_timeout(Duration::from_secs(5));
        released.store(true, Ordering::Relaxed);
        let first = first.unwrap();
        let Msg::Grepped {
            hits: Ok(hits),
            done: false,
            ..
        } = first
        else {
            panic!("not an intermediate hit")
        };
        assert_eq!(hits[0].root, late.path());
        handle.join().unwrap();
        let mut all = hits;
        for msg in msgs_rx.try_iter() {
            if let Msg::Grepped { hits: Ok(hits), .. } = msg {
                all.extend(hits);
            }
        }
        assert_eq!(all.len(), 2);
    }

    #[test]
    fn pending_hit_flushes_while_its_worker_is_stalled() {
        let dir = tree(&[("a", "x\nx\nx\n")]);
        let search_op = op(&[dir.path()], "x", 100);
        let released = Arc::new(AtomicBool::new(false));
        let release = released.clone();
        let (pending_tx, pending_rx) = mpsc::channel();
        let (msgs_tx, msgs_rx) = mpsc::channel();
        let handle = std::thread::spawn(move || {
            search_with_hook(
                &search_op,
                || false,
                |m| {
                    msgs_tx.send(m).unwrap();
                },
                |_, line| {
                    if line == 2 {
                        pending_tx.send(()).unwrap();
                        while !release.load(Ordering::Relaxed) {
                            std::thread::yield_now();
                        }
                    }
                },
            );
        });
        pending_rx.recv_timeout(Duration::from_secs(5)).unwrap();
        let first = msgs_rx.recv_timeout(Duration::from_secs(5)).unwrap();
        let second = msgs_rx.recv_timeout(Duration::from_secs(5));
        released.store(true, Ordering::Relaxed);
        let second = second.unwrap();
        assert_eq!(batches(&[first, second]), [(1, false), (1, false)]);
        handle.join().unwrap();
    }

    #[test]
    fn first_received_hits_fill_the_cap_before_a_stalled_root() {
        let (early, late) = (tree(&[("slow", "x\n")]), tree(&[("fast", "x\nx\nx\n")]));
        let early_path = early.path().join("slow");
        let search_op = with_workers(op(&[early.path(), late.path()], "x", 3), 2);
        let released = Arc::new(AtomicBool::new(false));
        let release = released.clone();
        let (started_tx, started_rx) = mpsc::channel();
        let (msgs_tx, msgs_rx) = mpsc::channel();
        let handle = std::thread::spawn(move || {
            search_with_hook(
                &search_op,
                || false,
                |m| {
                    msgs_tx.send(m).unwrap();
                },
                |path, line| {
                    if line == 0 && path == early_path {
                        started_tx.send(()).unwrap();
                        while !release.load(Ordering::Relaxed) {
                            std::thread::yield_now();
                        }
                    }
                },
            );
        });
        started_rx.recv_timeout(Duration::from_secs(5)).unwrap();
        let mut msgs = Vec::new();
        loop {
            let msg = msgs_rx.recv_timeout(Duration::from_secs(5));
            if let Ok(msg) = msg {
                let done = matches!(msg, Msg::Grepped { done: true, .. });
                msgs.push(msg);
                if done {
                    break;
                }
            } else {
                released.store(true, Ordering::Relaxed);
                handle.join().unwrap();
                panic!("cap completion waited for the stalled root");
            }
        }
        released.store(true, Ordering::Relaxed);
        handle.join().unwrap();
        assert_eq!(batches(&msgs).iter().map(|(n, _)| n).sum::<usize>(), 3);
        assert_eq!(batches(&msgs).iter().filter(|(_, done)| *done).count(), 1);
        assert!(
            msgs.into_iter()
                .flat_map(|m| match m {
                    Msg::Grepped { hits, .. } => hits.unwrap(),
                    _ => unreachable!(),
                })
                .all(|h| h.root == late.path())
        );
    }

    #[test]
    fn cancellation_while_waiting_for_a_hit_sends_nothing() {
        let dir = tree(&[("a", "x\n")]);
        let search_op = op(&[dir.path()], "x", 100);
        let cancelled = Arc::new(AtomicBool::new(false));
        let cancel = cancelled.clone();
        let released = Arc::new(AtomicBool::new(false));
        let release = released.clone();
        let (started_tx, started_rx) = mpsc::channel();
        let (done_tx, done_rx) = mpsc::channel();
        let handle = std::thread::spawn(move || {
            let mut msgs = Vec::new();
            search_with_hook(
                &search_op,
                || cancel.load(Ordering::Relaxed),
                |m| msgs.push(m),
                |_, line| {
                    if line == 0 {
                        started_tx.send(()).unwrap();
                        while !release.load(Ordering::Relaxed) {
                            std::thread::yield_now();
                        }
                    }
                },
            );
            done_tx.send(msgs).unwrap();
        });
        started_rx.recv_timeout(Duration::from_secs(5)).unwrap();
        cancelled.store(true, Ordering::Relaxed);
        released.store(true, Ordering::Relaxed);
        assert!(
            done_rx
                .recv_timeout(Duration::from_secs(5))
                .unwrap()
                .is_empty()
        );
        handle.join().unwrap();
    }

    #[test]
    fn an_active_producer_does_not_starve_the_deadline() {
        let dir = tree(&[("a", &"x\n".repeat(150))]);
        let msgs = std::sync::Mutex::new(Vec::new());
        search_with_hook(
            &op(&[dir.path()], "x", 200),
            || false,
            |m| {
                msgs.lock().unwrap().push(m);
            },
            |_, line| {
                if line > 0 {
                    std::thread::sleep(Duration::from_millis(1));
                }
            },
        );
        let batches = batches(&msgs.into_inner().unwrap());
        assert_eq!(batches[0], (1, false));
        assert!(batches.iter().filter(|(_, done)| !done).count() >= 2);
        assert_eq!(batches.iter().filter(|(_, done)| *done).count(), 1);
    }

    #[test]
    fn callback_unwind_releases_saturated_producers() {
        let dir = tree(&[("a", &"x\n".repeat(HELD * 10))]);
        let search_op = op(&[dir.path()], "x", 10_000);
        let (tx, rx) = mpsc::channel();
        let handle = std::thread::spawn(move || {
            let result = std::panic::catch_unwind(|| {
                search(&search_op, || false, |_| panic!("closed output"))
            });
            tx.send(result.is_err()).unwrap();
        });
        assert!(rx.recv_timeout(Duration::from_secs(5)).unwrap());
        handle.join().unwrap();
    }

    #[test]
    fn output_closure_releases_workers_without_completion() {
        let dir = tree(&[("a", &"x\n".repeat(HELD * 10))]);
        let search_op = op(&[dir.path()], "x", 10_000);
        let (done_tx, done_rx) = mpsc::channel();
        let handle = std::thread::spawn(move || {
            let (tx, mut rx) = tokio_mpsc::channel(1);
            search(
                &search_op,
                || tx.is_closed(),
                |msg| {
                    tx.blocking_send(msg).unwrap();
                    rx.close();
                },
            );
            done_tx.send(rx.try_recv().unwrap()).unwrap();
        });
        let msg = done_rx.recv_timeout(Duration::from_secs(5)).unwrap();
        assert_eq!(batches(&[msg]), [(1, false)]);
        handle.join().unwrap();
    }

    #[test]
    fn cap_terminates_with_competing_producers() {
        let names: Vec<String> = (0..40).map(|i| format!("f{i:02}")).collect();
        let body = "x\n".repeat(HELD * 3);
        let files: Vec<(&str, &str)> = names.iter().map(|n| (n.as_str(), body.as_str())).collect();
        let dir = tree(&files);
        let search_op = with_workers(op(&[dir.path()], "x", 1), 4);
        let (tx, rx) = mpsc::channel();
        let handle = std::thread::spawn(move || {
            let mut msgs = Vec::new();
            search(
                &search_op,
                || false,
                |msg| {
                    std::thread::sleep(Duration::from_millis(100));
                    msgs.push(msg);
                },
            );
            tx.send(msgs).unwrap();
        });
        assert_eq!(
            batches(&rx.recv_timeout(Duration::from_secs(5)).unwrap()),
            [(1, true)]
        );
        handle.join().unwrap();
    }

    #[test]
    fn a_cancel_with_a_saturated_result_queue_stops_workers() {
        let names: Vec<String> = (0..40).map(|i| format!("f{i:02}")).collect();
        let body = "x\n".repeat(HELD * 3);
        let files: Vec<(&str, &str)> = names.iter().map(|n| (n.as_str(), body.as_str())).collect();
        let dir = tree(&files);
        let search_op = with_workers(op(&[dir.path()], "x", 1 << 20), 4);
        let (tx, rx) = mpsc::channel();
        let handle = std::thread::spawn(move || {
            let cancel = AtomicBool::new(false);
            let mut sends = 0;
            search(
                &search_op,
                || cancel.load(Ordering::Relaxed),
                |_| {
                    sends += 1;
                    std::thread::sleep(Duration::from_millis(100));
                    cancel.store(true, Ordering::Relaxed);
                },
            );
            tx.send(sends).unwrap();
        });
        assert_eq!(rx.recv_timeout(Duration::from_secs(5)).unwrap(), 1);
        handle.join().unwrap();
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
