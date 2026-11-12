# Architecture review refactors, 2026-11-12

Benchmarks for the `architecture-review-refactors` change.

## Baseline

Run on `main` at `f0e4d13a398dedb39f36dee280ddd81f63fa8b7e` plus only task 1.1: the three new `hot_paths` cases, `Buffer::reparse` made public and returning whether it parsed, and `Buffer::marker_count`. Release builds, Rust 1.99.0, AMD Ryzen 7 5800X3D (16 logical CPUs, 31 GiB), Fedora 45, Linux 7.2.15, `performance` governor, desktop on mains power. `hot_paths` then `core`, sequentially, with the unchanged sampling helper. All self-checks passed.

Raw data: [hot_paths](baseline-hot_paths.tsv), [core](baseline-core.tsv). Times are medians of 21 warmed batches (single operations for prepared cases). This host is not the 2026-11-09 one (an Apple M5 Pro), so compare these numbers only with later runs here.

The new cases:

- `handle-tail/terminal-output-200-file-buffers`: one byte of `Msg::TerminalOutput` in a core with two watched projects, each indexing 100 files and 1,000 directories, and 200 visited files. The case asserts the watch set holds 2 roots, at least 200 files and 2,000 directories, and that the message returns no effects, so the time is the terminal feed plus `handle`'s tail.
- `hot-stream/prompt-typing-5000-line-transcript`: one self-insert in the prompt of the `session()` transcript (point at the end of the buffer). The transcript then holds **21,010 markers**, which the case asserts.
- `syntax-parse/rs/10000-lines-after-edit`: `Buffer::reparse` alone after one char typed in the middle of the `hot-scopes` 10,000-line Rust source; the case asserts the call parsed.

| Benchmark | Median |
| --- | ---: |
| `handle-tail/terminal-output-200-file-buffers` | 1.0 ms |
| `hot-stream/prompt-typing-5000-line-transcript` | 60.0 µs |
| `syntax-parse/rs/10000-lines-after-edit` | 39.3 µs |
| `syntax-edit-frame/rs/10000-lines-character-undo-pair` | 4.7 ms |
| `syntax-edit-frame/rs/10000-lines-comment-delimiter-undo-pair` | 48.2 ms |
| `syntax-edit-frame/rs/100000-lines-character-undo-pair` | 26.9 ms |
| `syntax-edit-frame/rs/100000-lines-comment-delimiter-undo-pair` | 485.2 ms |
| `syntax-edit-frame/tsx/10000-lines-character-undo-pair` | 6.8 ms |
| `syntax-edit-frame/tsx/10000-lines-comment-delimiter-undo-pair` | 15.0 ms |
| `syntax-edit-frame/tsx/100000-lines-character-undo-pair` | 9.6 ms |
| `syntax-edit-frame/tsx/100000-lines-comment-delimiter-undo-pair` | 59.4 ms |
| `revert-apply-frame/txt/100k-lines-one-line` | 237.2 µs |
| `revert-apply-frame/txt/100k-lines-scattered` | 1.6 ms |
| `revert-apply-frame/txt/100k-lines-replace-1000-line-block` | 282.3 µs |
| `revert-apply-frame/rs/100k-lines-one-line` | 187.7 ms |
| `revert-apply-frame/rs/100k-lines-scattered` | 251.2 ms |
| `revert-apply-frame/rs/100k-lines-replace-1000-line-block` | 199.6 ms |
| `frame/1-panes-shared-rust` | 384.8 µs |
| `frame/2-panes-shared-rust` | 772.4 µs |
| `frame/4-panes-shared-rust` | 1.5 ms |
| `frame/1-panes-distinct-rust` | 387.9 µs |
| `frame/2-panes-distinct-rust` | 781.4 µs |
| `frame/4-panes-distinct-rust` | 1.6 ms |
| `hot-stream/tool-output-shown-folded-500000` | 1.1 ms |
| `hot-stream/tool-output-shown-folded-5000000` | 1.1 ms |
| `hot-stream/tool-output-shown-open-500000` | 694.0 µs |
| `hot-stream/tool-output-shown-open-5000000` | 698.5 µs |
| `hot-stream/tool-output-hidden-500000` | 11.9 µs |
| `hot-stream/tool-output-hidden-5000000` | 12.0 µs |
| `hot-stream/shown-clean-frame` | 441.9 µs |
| `hot-stream/shown-dirty-frame` | 2.5 ms |
| `hot-stream/hidden-clean-frame` | 4.9 µs |
| `hot-stream/hidden-dirty-frame` | 13.2 µs |
| `hot-stream/unsorted-props-clean-frame` | 583.7 µs |
| `hot-stream/hidden-first-show-frame` | 2.5 ms |

Two readings for later tasks:

- The tail case is about 50 times D5's 20 µs threshold, so task 6.1 will take the profiling branch unless group 3 changes it.
- A one-char reparse of the 10k-line file costs 39 µs, a small part of the 4.7 ms character edit frame. The parse cost D2 moves off the thread is in edits that change much of the tree, such as the `/*` delimiter (48 ms at 10k lines, 485 ms at 100k).

## Parsing off the UI thread (tasks 4.4 and 4.5)

Run on `529ed63` plus group 4's changes (the parse service and the kept locals index), on the baseline host and setup, `core` then `hot_paths`, sequentially. All self-checks passed. Raw data: [core](parse-off-thread-core.tsv), [hot_paths](parse-off-thread-hot_paths.tsv).

Three families changed what they measure, so they compare with the baseline only as before and after:

- `syntax-edit-frame/*`, `revert-apply-frame/rs/*` and `hot-scopes/*` buffers are over 64 KiB, so their frames no longer parse; the parse goes out as an `Effect::Parse` that the case never delivers. `syntax-load/*` no longer parses either: `create_for_file` leaves an empty tree for the service.
- The benches' `open` helpers deliver a large file's first parse before a case acts, as the app does before the next key, so cases start from a coloured file as before.
- `syntax-parse/rs/10000-lines-after-edit` now times `Parse::run` on the edited snapshot, the work that left the frame; the buffers and the result are dropped outside the timed part.

| Benchmark | Baseline | Now |
| --- | ---: | ---: |
| `syntax-parse/rs/10000-lines-after-edit` | 39.3 µs | 36.1 µs |
| `syntax-edit-frame/rs/10000-lines-character-undo-pair` | 4.7 ms | 2.2 ms |
| `syntax-edit-frame/rs/10000-lines-comment-delimiter-undo-pair` | 48.2 ms | 2.2 ms |
| `syntax-edit-frame/rs/100000-lines-character-undo-pair` | 26.9 ms | 4.2 ms |
| `syntax-edit-frame/rs/100000-lines-comment-delimiter-undo-pair` | 485.2 ms | 4.3 ms |
| `syntax-edit-frame/tsx/100000-lines-character-undo-pair` | 9.6 ms | 6.8 ms |
| `syntax-edit-frame/tsx/100000-lines-comment-delimiter-undo-pair` | 59.4 ms | 6.8 ms |
| `hot-scopes/100000-lines-first-highlight-after-edit` | 13.4 ms | 2.5 ms |
| `hot-scopes/100000-lines-edit-highlight-undo-pair` | 26.5 ms | 4.1 ms |
| `revert-apply-frame/rs/100k-lines-one-line` | 187.7 ms | 181.9 ms |
| `revert-apply-frame/rs/100k-lines-scattered` | 251.2 ms | 193.5 ms |
| `frame/1-panes-shared-rust` | 384.8 µs | 395.7 µs |
| `frame/4-panes-distinct-rust` | 1.6 ms | 1.6 ms |
| `handle-tail/terminal-output-200-file-buffers` | 1.0 ms | 1.1 ms |
| `hot-stream/shown-dirty-frame` | 2.5 ms | 2.5 ms |

Readings:

- With the parse alone moved, the 100k-line character pair read 34.8 ms: each frame spent about 13 ms in the locals cache, 10 ms of it building two maps of all 108,041 fragments. The baseline's 26.9 ms was those frames, not the 39 µs parse. Task 4.4 keeps that index across rebuilds; the numbers above include it.
- An edit frame on the 100k-line Rust file is now 2.0–2.5 ms (the pair is two edits and two frames), from 13 ms. It is not under 2 ms: what is left is the root fragment re-queried over its 1,000 children, the locals walk, painting and the 0.4–0.5 ms a frame with no edit costs.
- A `/*` that recolours the rest of the file now costs what a one-char edit does.
- `revert-apply-frame/rs/*` stays near 190 ms: those costs are the revert's per-hunk edits, not the parse or the locals index.
- The 5–50% slowdowns the first run showed in `hot-stream/*` and `handle-tail/*` are gone in this sequential run (within 5%), so they were host noise.

## Session transcript as one block list (task 5.6)

Run on `3e9e8d0` (groups 1–5) on the baseline host and setup, `core` then `hot_paths`, sequentially. All self-checks passed. Raw data: [core](block-list-core.tsv), [hot_paths](block-list-hot_paths.tsv).

| Benchmark | Baseline | Now | Change |
| --- | ---: | ---: | ---: |
| `hot-stream/shown-clean-frame` | 441.9 µs | 462.3 µs | +4.6% |
| `hot-stream/hidden-clean-frame` | 4.9 µs | 4.9 µs | 0% |
| `hot-stream/unsorted-props-clean-frame` | 583.7 µs | 577.8 µs | −1.0% |
| `hot-stream/shown-dirty-frame` | 2.5 ms | 1.2 ms | −51.9% |
| `hot-stream/hidden-dirty-frame` | 13.2 µs | 13.4 µs | +1.7% |
| `hot-stream/hidden-first-show-frame` | 2.5 ms | 1.2 ms | −50.3% |
| `hot-stream/tool-output-shown-open-500000` | 694.0 µs | 701.6 µs | +1.1% |
| `hot-stream/tool-output-shown-open-5000000` | 698.5 µs | 696.9 µs | −0.2% |
| `hot-stream/tool-output-shown-folded-5000000` | 1.06 ms | 1.06 ms | −0.2% |
| `hot-stream/tool-output-hidden-500000` | 11.9 µs | 12.3 µs | +3.3% |
| `hot-stream/tool-output-hidden-5000000` | 12.0 µs | 13.6 µs | +13.4% |
| `session-render/5000-line-styled-message` | 110.6 ms | 105.4 ms | −4.7% |
| `hot-stream/prompt-typing-5000-line-transcript` | 60.0 µs | 52.8 µs | −12.1% |

Readings:

- Every clean, dirty-stream and first-show frame is within 10% of the baseline or faster; the dirty and first-show frames take half the time.
- `tool-output-hidden-5000000` is 1.6 µs slower (+13%); it is not one of the frames task 5.6 checks, and its 500,000-byte twin moved +3%.
- **Marker store input:** typing in the prompt of the 5,000-line transcript costs 52.8 µs and the buffer holds **21,010 markers**, the same count as the baseline, so the block list did not reduce the marker count, as the review's correction expected.

## Per-message tail (tasks 6.1, 6.2)

6.1: on `3e9e8d0` (groups 1–5), `handle-tail/terminal-output-200-file-buffers` took **1.05 ms** ([block-list hot_paths](block-list-hot_paths.tsv)), over D5's 20 µs. A `perf record --call-graph dwarf` of the case put the time in `watch_set`: building every file buffer's language-version candidate paths, sorting them, and cloning and comparing the indexed directories, on every message.

6.2: `Watch` keeps the last set behind a `dirty` flag. It is raised when a file buffer is made or killed (`Buffers::files_changed`), when a head is answered, on a project add, edit, write, reload or release, a workspace add or kill, an index request or reply, and new settings. In debug builds, a message that leaves the flag down asserts the kept set equals a fresh one. Five tests that write those fields directly now raise the flag too. Same host and setup, `hot_paths` only. Raw data: [hot_paths](watch-set-hot_paths.tsv).

| Benchmark | Before | Now |
| --- | ---: | ---: |
| `handle-tail/terminal-output-200-file-buffers` | 1.05 ms | **11.6 µs** |

The other cases in this run were 15–35% slower than the 5.6 run, `hot-stream/*` frames included. Frames don't call `handle`, so that is host noise, not this change.

## Final run (task 7.2)

Run on `3e9e8d0` plus group 6 (the whole change), on the baseline host and setup, `core`, `hot_paths` then `services`, sequentially, with the host otherwise idle (load average 2.7 at the start, falling). All self-checks passed. Raw data: [core](final-core.tsv), [hot_paths](final-hot_paths.tsv), [services](final-services.tsv).

Every `core` and `hot_paths` case is in the baseline. `services` was not run at baseline on this host (the 2026-11-09 run was an Apple M5 Pro), so its numbers are a first record here, not a comparison.

| Benchmark | Baseline | Final | Change |
| --- | ---: | ---: | ---: |
| `handle-tail/terminal-output-200-file-buffers` | 1.0 ms | 9.7 µs | −99.0% |
| `hot-stream/shown-dirty-frame` | 2.5 ms | 1.2 ms | −51.5% |
| `hot-stream/hidden-first-show-frame` | 2.5 ms | 1.2 ms | −51.3% |
| `hot-stream/shown-clean-frame` | 441.9 µs | 453.0 µs | +2.5% |
| `hot-stream/prompt-typing-5000-line-transcript` | 60.0 µs | 51.8 µs | −13.7% |
| `syntax-edit-frame/rs/10000-lines-comment-delimiter-undo-pair` | 48.2 ms | 2.2 ms | −95.5% |
| `syntax-edit-frame/rs/100000-lines-character-undo-pair` | 26.9 ms | 4.1 ms | −84.7% |
| `syntax-edit-frame/rs/100000-lines-comment-delimiter-undo-pair` | 485.2 ms | 4.1 ms | −99.1% |
| `syntax-edit-frame/tsx/100000-lines-comment-delimiter-undo-pair` | 59.4 ms | 6.7 ms | −88.7% |
| `hot-scopes/100000-lines-edit-highlight-undo-pair` | 26.5 ms | 4.2 ms | −84.2% |
| `revert-apply-frame/rs/100k-lines-scattered` | 251.2 ms | 186.8 ms | −25.7% |
| `session-render/5000-line-styled-message` | 110.6 ms | 106.2 ms | −4.0% |
| `frame/1-panes-shared-rust` | 384.8 µs | 393.1 µs | +2.1% |
| `frame/4-panes-distinct-rust` | 1.6 ms | 1.6 ms | −0.7% |

| Services benchmark | Final |
| --- | ---: |
| `index/10k-files-two-roots-warm-cache` | 16.5 ms |
| `grep/10k-hits-two-roots-complete-warm-cache` | 99.9 ms |
| `grep/first-batch-two-roots-warm-cache` | 585.4 µs |
| `grep/sparse-later-root-complete-warm-cache` | 80.1 ms |
| `grep/10k-hits-two-roots-complete-warm-cache-4-workers` | 30.8 ms |

Readings:

- `syntax-load/*` (−99.7 to −99.9%), `syntax-edit-frame/*`, `revert-apply-frame/rs/*` and `hot-scopes/*` changed what they measure in group 4 (see above): buffers over 64 KiB no longer parse on the UI thread. Their drops are the parse leaving the frame, not a faster parse.
- Regressions over 5%, all under 25 µs in absolute terms: `edit-undo/100k-lines` 340 → 365 ns (+7.5%), `terminal-display/50x120-screen` 133.8 → 141.2 µs (+5.6%), `finder/10000-paths-open-close` 1.0 → 1.1 ms (+5.5%) and `hot-stream/tool-output-hidden-5000000` 12.0 → 12.6 µs (+5.4%; +13% in the 5.6 run). None was repeated; the other 89 like-for-like cases are within 5% or faster.
- The 15–35% slowdowns in the 6.2 run are gone: that run shared the host with a game.

## Checks (task 7.1)

On f2276b2, cargo fmt --all --check, cargo clippy --all-targets -- -D warnings, cargo test, CI's aiditor-core dependency and panic-abort checks and scripts/test_benchmarks.py passed on both hosts:

- Linux: AMD Ryzen 7 5800X3D, Fedora 45, Rust 1.99.0. Tests: core 995, aiditor 59, services 105, scenarios and file_sync included; benchmark scripts 21.
- macOS: Apple M5 Pro, macOS 27.2.1, Rust 1.99.0. Tests: core 995, aiditor 59, services 105; benchmark scripts 21.
