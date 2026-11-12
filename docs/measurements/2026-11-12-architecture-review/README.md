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
