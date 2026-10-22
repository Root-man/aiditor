//! Project grep: the files the index lists, searched with ripgrep's crates on the blocking pool.

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use aiditor_core::{GrepOp, GrepSearch, Hit, Msg};
use grep_matcher::Matcher;
use grep_regex::RegexMatcherBuilder;
use grep_searcher::sinks::Lossy;
use grep_searcher::{BinaryDetection, SearcherBuilder};
use tokio::sync::mpsc;

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
    pub fn run(&self, op: GrepOp, tx: mpsc::Sender<Msg>) {
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

/// Searches `op`'s roots, sending `Msg::Grepped` batches until done or `op.limit` hits.
/// `cancelled` is checked before each file, after each match and before each send; once it is
/// true nothing more is sent.
pub fn search(op: &GrepSearch, cancelled: impl Fn() -> bool, mut send: impl FnMut(Msg)) {
    let mut reply = |hits, done| {
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
    let mut searcher = SearcherBuilder::new()
        .line_number(true)
        .binary_detection(BinaryDetection::quit(b'\0'))
        .build();
    let (mut hits, mut found, mut sent) = (Vec::new(), 0, Instant::now());
    'roots: for root in &op.roots {
        for file in crate::files::walk(root) {
            if cancelled() {
                return;
            }
            if found >= op.limit {
                break 'roots;
            }
            let path = file.strip_prefix(root).unwrap_or(&file);
            let sink = Lossy(|line, text: &str| {
                if cancelled() {
                    return Ok(false);
                }
                let text = text.strip_suffix('\n').unwrap_or(text);
                let text = text.strip_suffix('\r').unwrap_or(text);
                // shortcut: a non-UTF-8 line's lossy text may not match, giving column 0; match bytes if that matters.
                let start = (matcher.find(text.as_bytes()).ok().flatten()).map_or(0, |m| m.start());
                hits.push(Hit {
                    root: root.clone(),
                    path: path.to_path_buf(),
                    line: line as usize - 1,
                    col: text[..start].chars().count(),
                    text: text.to_string(),
                });
                found += 1;
                Ok(found < op.limit)
            });
            // Unreadable files are skipped, as `rg` skips them.
            _ = searcher.search_path(&matcher, &file, sink);
            if !hits.is_empty() && sent.elapsed() >= BATCH {
                if cancelled() {
                    return;
                }
                reply(Ok(std::mem::take(&mut hits)), false);
                sent = Instant::now();
            }
        }
    }
    if !cancelled() {
        reply(Ok(hits), true);
    }
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;
    use std::fs;
    use std::path::{Path, PathBuf};

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
            pattern: pattern.into(),
            limit,
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
        assert!(*done && hits.as_ref().is_err_and(|e| !e.is_empty()));
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
    fn hits_stream_in_batches_then_done() {
        let dir = tree(&[("a", "x\n"), ("b", "x\n"), ("c", "x\n")]);
        // Two checks per file, so each file takes a whole batch interval.
        let slow = || {
            std::thread::sleep(BATCH / 2);
            false
        };
        let mut msgs = Vec::new();
        search(&op(&[dir.path()], "x", 100), slow, |m| msgs.push(m));
        let expected = [(1, false), (1, false), (1, false), (0, true)];
        assert_eq!(batches(&msgs), expected);
    }

    #[test]
    fn a_search_cancelled_after_its_first_batch_sends_nothing_more() {
        let names: Vec<String> = (0..200).map(|i| format!("f{i:03}")).collect();
        let files: Vec<(&str, &str)> = names.iter().map(|n| (n.as_str(), "x\n")).collect();
        let dir = tree(&files);
        let (sent, after) = (Cell::new(false), Cell::new(0));
        let cancelled = || {
            std::thread::sleep(BATCH / 2);
            after.set(after.get() + usize::from(sent.get()));
            sent.get()
        };
        let mut msgs = Vec::new();
        search(&op(&[dir.path()], "x", 1 << 20), cancelled, |m| {
            sent.set(true);
            msgs.push(m);
        });
        // One batch, then the next file's check stops it.
        assert_eq!((batches(&msgs), after.get()), (vec![(1, false)], 1));
    }

    #[test]
    fn a_cancelled_search_stops_at_once_and_sends_nothing() {
        let names: Vec<String> = (0..200).map(|i| format!("f{i:03}")).collect();
        let files: Vec<(&str, &str)> = names.iter().map(|n| (n.as_str(), "x\n")).collect();
        let (many, big) = (tree(&files), tree(&[("big", &"x\n".repeat(10_000))]));
        for root in [many.path(), big.path()] {
            // Cancelled from the 11th check on, wherever the search is by then.
            let (checks, after) = (Cell::new(0), Cell::new(0));
            let cancelled = || {
                checks.set(checks.get() + 1);
                let cancelled = checks.get() > 10;
                after.set(after.get() + usize::from(cancelled));
                cancelled
            };
            let mut msgs = Vec::new();
            search(&op(&[root], "x", 1 << 20), cancelled, |m| {
                assert_eq!(after.get(), 0, "sent after the cancel");
                msgs.push(m);
            });
            let done = msgs
                .iter()
                .any(|m| matches!(m, Msg::Grepped { done: true, .. }));
            assert!(!done && after.get() <= 2, "{}", after.get());
        }
    }

    #[tokio::test]
    async fn a_newer_search_or_a_cancel_stops_the_running_one() {
        let names: Vec<String> = (0..2000).map(|i| format!("f{i:04}")).collect();
        let files: Vec<(&str, &str)> = names.iter().map(|n| (n.as_str(), "x\n")).collect();
        let (big, small) = (tree(&files), tree(&[("a", "x\n")]));
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
        grep.run(search(1, big.path()), tx.clone());
        grep.run(search(2, small.path()), tx.clone());
        while !done_with(&mut rx, &mut take, 2).await {}
        grep.run(search(3, big.path()), tx.clone());
        grep.run(GrepOp::Cancel, tx);
        // Ends once every search's task has dropped its sender.
        while let Some(msg) = rx.recv().await {
            take(msg);
        }
        assert_eq!(done, [2]);
    }

    /// Takes the next message, telling whether it is search `id`'s last.
    async fn done_with(rx: &mut mpsc::Receiver<Msg>, take: &mut impl FnMut(Msg), id: u64) -> bool {
        let msg = rx.recv().await.unwrap();
        let last = matches!(msg, Msg::Grepped { search, done: true, .. } if search == id);
        take(msg);
        last
    }
}
