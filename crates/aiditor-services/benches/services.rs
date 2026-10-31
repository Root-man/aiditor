use std::hint::black_box;
use std::num::NonZeroUsize;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use aiditor_core::{GrepSearch, Msg};
use aiditor_services::{files, grep};

#[path = "../../../benches/support.rs"]
mod support;

fn main() {
    let directory = tempfile::tempdir().unwrap();
    let roots: Vec<_> = ["a", "b"].map(|name| directory.path().join(name)).into();
    let body = format!("{}needle été\n", "ordinary source line\n".repeat(50));
    for root in &roots {
        std::fs::create_dir_all(root.join("ignored")).unwrap();
        std::fs::create_dir_all(root.join("generated")).unwrap();
        std::fs::write(root.join(".gitignore"), "ignored/\n").unwrap();
        for ignored in ["ignored/x.txt", "generated/x.txt"] {
            std::fs::write(root.join(ignored), "needle\n").unwrap();
        }
        for dir in 0..50 {
            let folder = root.join(format!("src{dir:03}"));
            std::fs::create_dir(&folder).unwrap();
            for file in 0..100 {
                std::fs::write(folder.join(format!("file{file:03}.txt")), &body).unwrap();
            }
        }
    }
    std::fs::write(roots[1].join("src049/file099.txt"), format!("{body}rare\n")).unwrap();
    let ignore: Vec<String> = vec!["generated/".into()];
    let patterns = files::Patterns::new(&ignore).unwrap();
    let index = || {
        roots
            .iter()
            .map(|root| files::index(root, &patterns).unwrap())
            .collect::<Vec<_>>()
    };
    let indexed = index();
    for names in &indexed {
        assert_eq!(names.len(), 5_001);
        assert!(names.contains(&PathBuf::from("src049/file099.txt")));
        assert!(names.contains(&PathBuf::from(".gitignore")));
    }
    let mut output = String::new();
    support::measure(
        "index/10k-files-two-roots-warm-cache",
        || {
            black_box(index());
        },
        &mut output,
    );
    let search = |workers| GrepSearch {
        id: 1,
        roots: roots.clone(),
        ignore: ignore.clone(),
        pattern: "needle".into(),
        limit: 10_000,
        workers: NonZeroUsize::new(workers).unwrap(),
    };
    let run = |search: &GrepSearch, first_only: bool| -> (Duration, Duration) {
        let stop = AtomicBool::new(false);
        let (mut count, mut done_seen) = (0, false);
        let (start, mut first) = (Instant::now(), None);
        grep::search(
            search,
            || stop.load(Ordering::Relaxed),
            |msg| {
                let Msg::Grepped { hits, done, .. } = msg else {
                    unreachable!()
                };
                let hits = hits.unwrap();
                for hit in &hits {
                    assert_eq!(
                        hit.text,
                        if search.pattern == "rare" {
                            "rare"
                        } else {
                            "needle été"
                        }
                    );
                    assert_eq!(hit.line, if search.pattern == "rare" { 51 } else { 50 });
                    assert_eq!(hit.col, 0);
                }
                count += hits.len();
                if count > 0 && first.is_none() {
                    first = Some(start.elapsed());
                }
                done_seen |= done;
                if first_only && count > 0 {
                    stop.store(true, Ordering::Relaxed);
                }
                black_box(hits);
            },
        );
        assert!(count > 0);
        if !first_only {
            assert_eq!(count, if search.pattern == "rare" { 1 } else { 10_000 });
            assert!(done_seen);
        }
        (first.unwrap(), start.elapsed())
    };
    // One worker keeps the names of the measurements from before workers.
    for (workers, suffix) in [(1, ""), (4, "-4-workers")] {
        let search = search(workers);
        run(&search, false);
        support::measure(
            &format!("grep/10k-hits-two-roots-complete-warm-cache{suffix}"),
            || {
                run(&search, false);
            },
            &mut output,
        );
        // Cancelling in the first callback includes the small cost of unwinding the search.
        support::prepared(
            &format!("grep/first-batch-two-roots-warm-cache{suffix}"),
            || (),
            |_| {
                run(&search, true);
            },
            &mut output,
        );
        let sparse = GrepSearch {
            id: 1,
            roots: roots.clone(),
            ignore: ignore.clone(),
            pattern: "rare".into(),
            limit: 10_000,
            workers: search.workers,
        };
        for _ in 0..3 {
            run(&sparse, false);
        }
        let (mut first, mut complete) = (Vec::new(), Vec::new());
        for _ in 0..21 {
            let (a, b) = run(&sparse, false);
            first.push(a.as_secs_f64() * 1e9);
            complete.push(b.as_secs_f64() * 1e9);
        }
        record(
            &format!("grep/sparse-later-root-first-callback-warm-cache{suffix}"),
            first,
            &mut output,
        );
        record(
            &format!("grep/sparse-later-root-complete-warm-cache{suffix}"),
            complete,
            &mut output,
        );
    }
    if let Some(path) = std::env::var_os("BENCH_OUTPUT") {
        std::fs::write(path, output).expect("write benchmark results");
    }
}

fn record(name: &str, mut samples: Vec<f64>, output: &mut String) {
    samples.sort_by(f64::total_cmp);
    println!(
        "{name}: {:.0} ns/op (median of 21 single-operation batches)",
        samples[10]
    );
    output.push_str(&format!(
        "{name}\t{}\t{}\t{}\n",
        samples[10], samples[0], samples[20]
    ));
}
