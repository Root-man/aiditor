# Refactor scan fixes, 2026-11-09

Benchmarks for the `fix-m6-refactor-scan-findings` change.

## Baseline

Run on `main` at `0f74a9b9219569876949534299dde3ab912ae43d` plus only the new `hot-stream/unsorted-props-clean-frame` case (task 1.1), in a clean worktree, release builds, Rust 1.99.0, Apple M5 Pro (18 logical CPUs, 48 GiB), macOS 27.2 (26B123), power mode `0`, **on battery** (the 2026-11-08 run was on AC). `hot_paths` then `core`, sequentially, with the unchanged sampling helper. All self-checks passed.

Raw data: [hot_paths](baseline-hot_paths.tsv), [core](baseline-core.tsv). Times are medians of 21 warmed batches (single operations for prepared cases).

The new case is the existing `session()` transcript followed by three read calls with a thought between each pair, which form a group whose thought rows add their properties out of order; the group is opened so the frame shows the thought rows, which the case asserts before and after timing.

| Benchmark | Median |
| --- | ---: |
| `hot-stream/unsorted-props-clean-frame` | 56.688 ms |
| `hot-stream/shown-clean-frame` | 213.1 µs |
| `hot-stream/shown-dirty-frame` | 45.199 ms |
| `hot-stream/hidden-first-show-frame` | 44.929 ms |
| `hot-stream/tool-output-shown-open-500000` | 3.379 ms |
| `hot-stream/tool-output-shown-open-5000000` | 14.986 ms |
| `hot-stream/tool-output-shown-folded-500000` | 529.6 µs |
| `hot-stream/tool-output-shown-folded-5000000` | 566.8 µs |
| `terminal-copy/colored/10k-lines-display` | 290.3 µs |
| `terminal-copy/plain/10k-lines-display` | 59.0 µs |
| `display/50-rows-in-100k-lines` | 75.3 µs |
| `frame/1-panes-shared-rust` | 169.3 µs |
| `syntax-display/rs/10000-lines-stationary` | 218.4 µs |

The unsorted clean frame costs about 266 times the sorted one, matching the scan's 51–55 ms probe.

## After tasks 1.1–5.1

Same host, battery and sequence, on branch `fix-m6-refactor-scan-findings` with the property index (2.1–2.3), the display and per-line query fixes (3.1–3.3), open-call appends (4.1–4.2) and the markdown stable point (5.1, not yet used by streaming). Raw data: [hot_paths](tasks-1-10-hot_paths.tsv), [core](tasks-1-10-core.tsv). All self-checks passed.

| Benchmark | Baseline | After | Change |
| --- | ---: | ---: | ---: |
| `display/50-rows-in-100k-lines` | 75.3 µs | 64.3 µs | -14.5% |
| `frame/1-panes-distinct-rust` | 165.9 µs | 149.1 µs | -10.1% |
| `frame/1-panes-shared-rust` | 169.3 µs | 155.9 µs | -7.9% |
| `frame/2-panes-distinct-rust` | 335.2 µs | 306.3 µs | -8.6% |
| `frame/2-panes-shared-rust` | 333.8 µs | 309.3 µs | -7.3% |
| `frame/4-panes-distinct-rust` | 669.9 µs | 617.2 µs | -7.9% |
| `frame/4-panes-shared-rust` | 679.7 µs | 608.9 µs | -10.4% |
| `hot-stream/hidden-clean-frame` | 1.8 µs | 1.8 µs | -0.0% |
| `hot-stream/hidden-dirty-frame` | 6.2 µs | 6.9 µs | +10.7% |
| `hot-stream/hidden-first-show-frame` | 44.929 ms | 47.667 ms | +6.1% |
| `hot-stream/shown-clean-frame` | 213.1 µs | 196.5 µs | -7.8% |
| `hot-stream/shown-dirty-frame` | 45.199 ms | 47.408 ms | +4.9% |
| `hot-stream/tool-output-hidden-500000` | 6.9 µs | 6.6 µs | -4.2% |
| `hot-stream/tool-output-hidden-5000000` | 7.2 µs | 7.2 µs | +1.2% |
| `hot-stream/tool-output-shown-folded-500000` | 529.6 µs | 533.5 µs | +0.7% |
| `hot-stream/tool-output-shown-folded-5000000` | 566.8 µs | 551.1 µs | -2.8% |
| `hot-stream/tool-output-shown-open-500000` | 3.379 ms | 277.7 µs | -91.8% |
| `hot-stream/tool-output-shown-open-5000000` | 14.986 ms | 281.8 µs | -98.1% |
| `hot-stream/unsorted-props-clean-frame` | 56.687 ms | 245.6 µs | -99.6% |
| `terminal-copy/colored/10k-lines-display` | 290.3 µs | 222.0 µs | -23.5% |
| `terminal-copy/colored/10k-lines-enter-exit-pair` | 56.940 ms | 60.492 ms | +6.2% |
| `terminal-copy/colored/10k-lines-scroll-pair` | 4.017 ms | 622.3 µs | -84.5% |
| `terminal-copy/plain/10k-lines-display` | 59.0 µs | 53.2 µs | -9.9% |
| `terminal-copy/plain/10k-lines-enter-exit-pair` | 14.836 ms | 14.515 ms | -2.2% |
| `terminal-copy/plain/10k-lines-scroll-pair` | 206.0 µs | 188.5 µs | -8.5% |

- The unsorted clean frame (245.6 µs) now costs about the sorted one (196.5 µs) instead of 56.7 ms.
- Open tool output past the cut appends in about 0.28 ms whatever its size, below the folded cases (0.53–0.55 ms).
- `terminal-copy/*/10k-lines-display` fell to 222.0 µs colored and 53.2 µs plain, near #404's 204 and 52 µs.
- **Slower:** the dirty stream frames (+5–6%, about 2.3 ms) and the colored copy-mode enter/exit pair (+6%, 3.6 ms) pay for keeping the index's `reach` up to date as properties are added and removed. `hot-stream/hidden-dirty-frame` (+0.7 µs) and `revert-apply-frame/rs/100k-lines-scattered` (+5.7%) are within this machine's run-to-run noise. Task 5.2 targets the dirty frames.
- A first version that renumbered the whole index on every insert in the middle doubled the dirty frames (87 ms); the index now stays implicit while properties are in order.
