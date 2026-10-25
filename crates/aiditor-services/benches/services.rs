use std::cell::Cell;
use std::hint::black_box;
use std::path::PathBuf;

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
    let ignore = vec!["generated/".into()];
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
    let search = GrepSearch {
        id: 1,
        roots,
        ignore,
        pattern: "needle".into(),
        limit: 10_000,
    };
    let run = |first_only: bool| {
        let stop = Cell::new(false);
        let (mut count, mut done_seen) = (0, false);
        grep::search(
            &search,
            || stop.get(),
            |msg| {
                let Msg::Grepped { hits, done, .. } = msg else {
                    unreachable!()
                };
                let hits = hits.unwrap();
                for hit in &hits {
                    assert_eq!(hit.text, "needle été");
                    assert_eq!(hit.line, 50);
                    assert_eq!(hit.col, 0);
                }
                count += hits.len();
                done_seen |= done;
                if first_only && count > 0 {
                    stop.set(true);
                }
                black_box(hits);
            },
        );
        assert!(count > 0);
        if !first_only {
            assert_eq!(count, 10_000);
            assert!(done_seen);
        }
    };
    run(false);
    support::measure(
        "grep/10k-hits-two-roots-complete-warm-cache",
        || run(false),
        &mut output,
    );
    // Cancelling in the first callback includes the small cost of unwinding the search.
    support::prepared(
        "grep/first-batch-two-roots-warm-cache",
        || (),
        |_| run(true),
        &mut output,
    );
    if let Some(path) = std::env::var_os("BENCH_OUTPUT") {
        std::fs::write(path, output).expect("write benchmark results");
    }
}
