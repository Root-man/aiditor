# Full benchmark suite, 2026-11-08

Run on `main` at `0f74a9b9219569876949534299dde3ab912ae43d` (52 commits after PR #404's `34f6a0b`), clean worktree, release builds, Rust 1.99.0, Apple M5 Pro (18 logical CPUs, 48 GiB), macOS 27.2 (26B123), AC power, power mode `0`, built-in Liquid Retina XDR display (3456×2234 physical, reported 120 Hz). Those 52 commits are mostly agent-buffer work from the interactive agent session milestone.

Everything ran sequentially with the unchanged sampling helper. One benchmark source had changed since #404: #457 (`3819d1f`) added a markdown table to the `hot-stream` fixture, so the `hot-stream/shown-*` and `hidden-first-show-frame` comparisons with #404 are not like-for-like (see the [2026-11-09 scan fixes](../2026-11-09-scan-fixes/README.md)). Suites: `core` (69 cases), `hot_paths` (20), `services` (9) and the GUI suite (`python3 scripts/bench_local.py --suite ui`). All self-checks passed; no harness repairs were needed (the assertion drift fixed for #404 is gone). Core and `hot_paths` were run a second time to check noise: [core repeat](core-repeat.tsv), [hot_paths repeat](hot_paths-repeat.tsv). Only differences above 8% between the two core runs are `display/50-rows-in-100k-lines`, `terminal-output/64k-batch-of-cat`, `syntax-load/rs/10000-lines-warm-grammar` and `syntax-edit-frame/tsx/100000-lines-comment-delimiter-undo-pair` (all 9%).

Raw data: [core](core.tsv), [hot_paths](hot_paths.tsv), [services](services.tsv), [GUI samples](ui.tsv), [GUI report](ui.md). The baseline is the **after** column of [PR #404's report](../2026-11-01-hot-paths/README.md) (`after-core.tsv`, `after-focused.tsv`, `after-services.tsv`), measured on the same host with macOS 27.0.1. Times are per operation: median, minimum and maximum of 21 warmed batches (single operations for prepared cases), not latency percentiles or confidence intervals. Positive change means slower. Headless frame construction excludes GPUI shaping and painting; filesystem cases use warm caches. Nothing here is a CI gate.

## Summary

- **Display and frame construction is slower; edit, parse and load paths are not.** 13 of 69 core cases are more than 10% slower than #404 and none is more than 10% faster. All 13 build frames or displays: the Rust/TSX/JS `syntax-display` stationary and scroll cases (+10–17%), `frame/*-panes-shared-rust` (+11–12%), `terminal-copy` display and scroll (+28–41%, but 0.2–0.3 ms) and `display/50-rows-in-100k-lines` (+31%, 54.5 → 71.4 µs; +9% between this run and its repeat). Buffer load, edit/undo, syntax load, revert diff, finder and terminal output are within about ±10% either way. The absolute costs are still sub-millisecond per frame.
- **Agent-buffer frame cost rose.** `hot-stream/shown-clean-frame` (a clean frame of a roughly 5,000-line styled reply) went 126 → 211 µs (+67%), and the cached-frame cases for scoped Rust buffers rose about 10% (194 → 216 µs at 10k lines, 201 → 220 µs at 100k). The repeat run gave 220, 226 and 233 µs, so this is stable. No cause was profiled here; a bisect over the 52 commits is the next step if it matters.
- **Dirty-stream paths improved.** Shown dirty frames and first show of a hidden dirty buffer fell from about 49.9 to 43.9 ms (-12%); the hidden dirty frame stays near zero (4.7 µs).
- **New: tool-output rendering.** Six new `hot-stream/tool-output-*` cases: a folded tool call costs about 0.5 ms regardless of output size (500 KB or 5 MB); an open one costs 3.3 ms at 500,000 characters and 14.7 ms at 5,000,000; hidden ones cost about 6 µs.
- **Grep got faster:** about -10% on complete searches and sparse first callbacks, -18% on the four-worker sparse first callback (222–311 ms spread, so the least certain). First batch is unchanged. The host differs only by an OS point release, but file-open cost varies between runs on this machine (see [performance.md](../../performance.md)).
- **GUI input-to-paint is unchanged:** baseline p50 23.8 ms (24.3 and 23.7 in #404), flood scenarios p50 about 16.5 ms.

## GUI input during terminal floods

120 input samples per scenario, default window options, temporary PTY fixtures. Actual window bounds were not sampled. Input-to-paint covers scheduled synthetic input through the matching frame's paint callback, excluding OS keyboard delivery, GPU completion and scanout. Paint gaps include input cadence and idle time.

```
Scenario / metric | Samples | p50 | p95 | p99 | Max
baseline / input-to-paint | 120 | 23.808 ms | 27.499 ms | 28.481 ms | 28.977 ms
baseline / paint-gap | 119 | 73.900 ms | 77.739 ms | 78.521 ms | 79.006 ms
baseline: 0 terminal bytes processed during measurement
yes / input-to-paint | 120 | 16.564 ms | 17.056 ms | 17.722 ms | 28.125 ms
yes / paint-gap | 942 | 8.334 ms | 14.665 ms | 15.260 ms | 26.660 ms
yes: 166282611 terminal bytes processed during measurement
cat / input-to-paint | 120 | 16.422 ms | 17.021 ms | 17.215 ms | 17.293 ms
cat / paint-gap | 927 | 8.334 ms | 14.409 ms | 14.992 ms | 16.004 ms
cat: 392400499 terminal bytes processed during measurement
```

| Scenario | #404 after (p50 / p95 / p99 / max ms) | Current (p50 / p95 / p99 / max ms) |
| --- | --- | --- |
| baseline | 23.708 / 27.958 / 28.631 / 28.713 | 23.808 / 27.499 / 28.481 / 28.977 |
| yes | 8.374 / 16.738 / 16.920 / 17.008 | 16.564 / 17.056 / 17.722 / 28.125 |
| cat | 8.396 / 16.824 / 16.895 / 26.516 | 16.422 / 17.021 / 17.215 / 17.293 |

The yes and cat p50 doubled from #404's 8.4 ms to 16.5 ms. The earlier supplied baseline had 16.7 ms and the #404 runs 8.4–9.0 ms, with p95 near 16.8 ms in all of them, so the p50 looks bimodal (likely one versus two 120 Hz frames; not verified). The current runs sit in the upper mode. Their p95 and p99 are within 1 ms of #404, and the tail is tighter for cat (max 17.3 ms); yes has one 28.1 ms sample. 120 samples cannot separate a real shift from that bimodality.

## Core results ([#404 after](../2026-11-01-hot-paths/after-core.tsv))

| Benchmark | #404 after | Current median | Min | Max | Change |
| --- | ---: | ---: | ---: | ---: | ---: |
| `buffer-load/100k-lines` | 539.841 µs | 527.862 µs | 521.984 µs | 721.229 µs | -2.2% |
| `edit-undo/100k-lines` | 133.5 ns | 121.3 ns | 119.5 ns | 125.0 ns | -9.1% |
| `display/50-rows-in-100k-lines` | 54.545 µs | 71.364 µs | 69.869 µs | 73.135 µs | +30.8% |
| `swiper/two-queries-in-100k-lines` | 6.831 ms | 7.051 ms | 6.894 ms | 7.401 ms | +3.2% |
| `terminal-output/64k-batch-of-yes` | 641.338 µs | 675.141 µs | 661.255 µs | 684.422 µs | +5.3% |
| `terminal-output/64k-batch-of-cat` | 339.197 µs | 341.579 µs | 337.402 µs | 406.993 µs | +0.7% |
| `terminal-display/50x120-screen` | 64.518 µs | 58.854 µs | 54.347 µs | 65.654 µs | -8.8% |
| `syntax-load/rs/10000-lines-warm-grammar` | 26.801 ms | 24.683 ms | 24.354 ms | 25.396 ms | -7.9% |
| `syntax-edit-frame/rs/10000-lines-character-undo-pair` | 2.188 ms | 2.196 ms | 2.171 ms | 2.256 ms | +0.4% |
| `syntax-edit-frame/rs/10000-lines-comment-delimiter-undo-pair` | 19.203 ms | 18.336 ms | 18.131 ms | 19.433 ms | -4.5% |
| `syntax-display/rs/10000-lines-stationary` | 183.757 µs | 208.385 µs | 204.983 µs | 213.234 µs | +13.4% |
| `syntax-display/rs/10000-lines-scroll-pair` | 751.682 µs | 812.667 µs | 798.620 µs | 875.169 µs | +8.1% |
| `syntax-load/rs/100000-lines-warm-grammar` | 275.990 ms | 264.575 ms | 258.733 ms | 275.867 ms | -4.1% |
| `syntax-edit-frame/rs/100000-lines-character-undo-pair` | 14.172 ms | 13.211 ms | 13.040 ms | 13.550 ms | -6.8% |
| `syntax-edit-frame/rs/100000-lines-comment-delimiter-undo-pair` | 196.587 ms | 190.871 ms | 185.448 ms | 203.309 ms | -2.9% |
| `syntax-display/rs/100000-lines-stationary` | 195.021 µs | 227.334 µs | 220.806 µs | 236.329 µs | +16.6% |
| `syntax-display/rs/100000-lines-scroll-pair` | 776.458 µs | 856.331 µs | 833.148 µs | 897.797 µs | +10.3% |
| `syntax-load/tsx/10000-lines-warm-grammar` | 33.820 ms | 32.724 ms | 32.231 ms | 35.780 ms | -3.2% |
| `syntax-edit-frame/tsx/10000-lines-character-undo-pair` | 2.734 ms | 2.753 ms | 2.708 ms | 2.876 ms | +0.7% |
| `syntax-edit-frame/tsx/10000-lines-comment-delimiter-undo-pair` | 5.619 ms | 5.497 ms | 5.393 ms | 5.836 ms | -2.2% |
| `syntax-display/tsx/10000-lines-stationary` | 208.867 µs | 234.329 µs | 229.802 µs | 239.986 µs | +12.2% |
| `syntax-display/tsx/10000-lines-scroll-pair` | 1.102 ms | 1.115 ms | 1.105 ms | 1.155 ms | +1.2% |
| `syntax-load/tsx/100000-lines-warm-grammar` | 353.598 ms | 340.469 ms | 333.641 ms | 350.671 ms | -3.7% |
| `syntax-edit-frame/tsx/100000-lines-character-undo-pair` | 4.047 ms | 4.006 ms | 3.975 ms | 4.100 ms | -1.0% |
| `syntax-edit-frame/tsx/100000-lines-comment-delimiter-undo-pair` | 20.551 ms | 19.639 ms | 19.116 ms | 20.053 ms | -4.4% |
| `syntax-display/tsx/100000-lines-stationary` | 244.471 µs | 275.178 µs | 271.529 µs | 293.336 µs | +12.6% |
| `syntax-display/tsx/100000-lines-scroll-pair` | 1.163 ms | 1.217 ms | 1.213 ms | 1.235 ms | +4.7% |
| `syntax-load/js/10000-lines-warm-grammar` | 23.070 ms | 22.121 ms | 21.921 ms | 22.519 ms | -4.1% |
| `syntax-load/ts/10000-lines-warm-grammar` | 17.235 ms | 16.445 ms | 16.260 ms | 17.038 ms | -4.6% |
| `syntax-load/ex/10000-lines-warm-grammar` | 42.875 ms | 41.195 ms | 40.779 ms | 44.126 ms | -3.9% |
| `syntax-load/heex/10000-lines-warm-grammar` | 56.128 ms | 51.905 ms | 51.521 ms | 52.227 ms | -7.5% |
| `syntax-load/yaml/10000-lines-warm-grammar` | 32.957 ms | 30.387 ms | 29.756 ms | 32.654 ms | -7.8% |
| `syntax-load/toml/10000-lines-warm-grammar` | 16.933 ms | 16.328 ms | 15.871 ms | 17.693 ms | -3.6% |
| `syntax-load/md/10000-lines-warm-grammar` | 31.657 ms | 29.718 ms | 29.205 ms | 34.274 ms | -6.1% |
| `syntax-display/js/100kb-single-line` | 201.409 µs | 230.455 µs | 228.812 µs | 237.242 µs | +14.4% |
| `syntax-display/rs/1mb-single-line-at-end` | 7.644 µs | 7.316 µs | 7.199 µs | 7.791 µs | -4.3% |
| `syntax-display/rs/1mb-single-line-wrapped-at-end` | 295.930 µs | 309.381 µs | 308.076 µs | 315.248 µs | +4.5% |
| `finder/10000-paths-open-close` | 525.262 µs | 478.052 µs | 468.816 µs | 485.901 µs | -9.0% |
| `finder/10000-paths-narrow-broaden-pair` | 752.388 µs | 703.227 µs | 656.482 µs | 765.497 µs | -6.5% |
| `finder/100000-paths-open-close` | 5.274 ms | 4.777 ms | 4.690 ms | 4.916 ms | -9.4% |
| `finder/100000-paths-narrow-broaden-pair` | 3.321 ms | 3.637 ms | 3.491 ms | 4.119 ms | +9.5% |
| `revert-diff/txt/100k-lines-one-line` | 11.703 ms | 11.541 ms | 11.467 ms | 11.753 ms | -1.4% |
| `revert-apply-frame/txt/100k-lines-one-line` | 118.125 µs | 123.791 µs | 117.708 µs | 128.625 µs | +4.8% |
| `revert-diff/txt/100k-lines-scattered` | 16.817 ms | 16.501 ms | 16.392 ms | 16.902 ms | -1.9% |
| `revert-apply-frame/txt/100k-lines-scattered` | 594.042 µs | 594.125 µs | 569.041 µs | 644.792 µs | +0.0% |
| `revert-diff/txt/100k-lines-replace-1000-line-block` | 13.564 ms | 13.395 ms | 13.294 ms | 13.464 ms | -1.2% |
| `revert-apply-frame/txt/100k-lines-replace-1000-line-block` | 140.375 µs | 148.166 µs | 143.292 µs | 159.042 µs | +5.6% |
| `revert-diff/rs/100k-lines-one-line` | 11.659 ms | 11.530 ms | 11.465 ms | 11.765 ms | -1.1% |
| `revert-apply-frame/rs/100k-lines-one-line` | 69.780 ms | 64.669 ms | 63.933 ms | 69.014 ms | -7.3% |
| `revert-diff/rs/100k-lines-scattered` | 16.959 ms | 16.581 ms | 16.424 ms | 16.777 ms | -2.2% |
| `revert-apply-frame/rs/100k-lines-scattered` | 101.619 ms | 97.936 ms | 96.226 ms | 105.124 ms | -3.6% |
| `revert-diff/rs/100k-lines-replace-1000-line-block` | 13.653 ms | 13.424 ms | 13.325 ms | 13.661 ms | -1.7% |
| `revert-apply-frame/rs/100k-lines-replace-1000-line-block` | 74.387 ms | 69.228 ms | 68.640 ms | 71.896 ms | -6.9% |
| `terminal-copy/plain/10k-lines-enter-exit-pair` | 15.142 ms | 13.901 ms | 13.249 ms | 14.444 ms | -8.2% |
| `terminal-copy/plain/10k-lines-display` | 52.346 µs | 57.567 µs | 56.931 µs | 59.110 µs | +10.0% |
| `terminal-copy/plain/10k-lines-scroll-pair` | 147.719 µs | 195.788 µs | 193.988 µs | 199.342 µs | +32.5% |
| `terminal-copy/colored/10k-lines-enter-exit-pair` | 58.260 ms | 56.576 ms | 55.687 ms | 58.871 ms | -2.9% |
| `terminal-copy/colored/10k-lines-display` | 203.845 µs | 286.805 µs | 285.592 µs | 289.353 µs | +40.7% |
| `terminal-copy/colored/10k-lines-scroll-pair` | 3.123 ms | 3.994 ms | 3.977 ms | 4.727 ms | +27.9% |
| `terminal-output/ansi-tui-osc-trace` | 431.214 µs | 412.348 µs | 406.827 µs | 423.030 µs | -4.4% |
| `terminal-display/ansi-unicode-50x120` | 74.471 µs | 67.676 µs | 64.902 µs | 73.146 µs | -9.1% |
| `terminal-resize/10k-lines-120-to-80-columns` | 2.080 ms | 2.093 ms | 1.471 ms | 2.455 ms | +0.6% |
| `frame/1-panes-shared-rust` | 147.984 µs | 164.115 µs | 161.407 µs | 171.904 µs | +10.9% |
| `frame/2-panes-shared-rust` | 286.721 µs | 321.764 µs | 315.736 µs | 331.613 µs | +12.2% |
| `frame/4-panes-shared-rust` | 572.369 µs | 642.685 µs | 635.599 µs | 659.010 µs | +12.3% |
| `frame/1-panes-distinct-rust` | 144.008 µs | 156.974 µs | 154.718 µs | 165.808 µs | +9.0% |
| `frame/2-panes-distinct-rust` | 285.809 µs | 312.835 µs | 310.458 µs | 324.371 µs | +9.5% |
| `frame/4-panes-distinct-rust` | 603.361 µs | 642.016 µs | 632.865 µs | 673.763 µs | +6.4% |
| `session-render/5000-line-styled-message` | 48.655 ms | 47.151 ms | 46.842 ms | 47.783 ms | -3.1% |

## Focused hot paths ([#404 after](../2026-11-01-hot-paths/after-focused.tsv))

| Benchmark | #404 after | Current median | Min | Max | Change |
| --- | ---: | ---: | ---: | ---: | ---: |
| `hot-scopes/10000-lines-first-highlight-after-edit` | 1.142 ms | 1.117 ms | 1.050 ms | 1.170 ms | -2.3% |
| `hot-scopes/10000-lines-edit-highlight-undo-pair` | 2.157 ms | 2.144 ms | 2.115 ms | 2.166 ms | -0.6% |
| `hot-scopes/10000-lines-cached-frame` | 193.694 µs | 215.671 µs | 213.152 µs | 232.053 µs | +11.3% |
| `hot-scopes/100000-lines-first-highlight-after-edit` | 7.018 ms | 6.978 ms | 6.763 ms | 7.425 ms | -0.6% |
| `hot-scopes/100000-lines-edit-highlight-undo-pair` | 13.558 ms | 13.104 ms | 12.900 ms | 13.810 ms | -3.4% |
| `hot-scopes/100000-lines-cached-frame` | 200.832 µs | 220.098 µs | 218.270 µs | 225.259 µs | +9.6% |
| `hot-preview/100000-matches-stationary` | 7.827 ms | 7.683 ms | 7.619 ms | 7.752 ms | -1.8% |
| `hot-preview/1000000-matches-stationary` | 74.576 ms | 72.966 ms | 72.661 ms | 76.004 ms | -2.2% |
| `hot-stream/tool-output-shown-folded-500000` | — | 525.875 µs | 517.333 µs | 585.291 µs | new |
| `hot-stream/tool-output-shown-open-500000` | — | 3.286 ms | 3.159 ms | 3.463 ms | new |
| `hot-stream/tool-output-hidden-500000` | — | 5.917 µs | 5.167 µs | 6.791 µs | new |
| `hot-stream/tool-output-shown-folded-5000000` | — | 547.167 µs | 535.083 µs | 582.334 µs | new |
| `hot-stream/tool-output-shown-open-5000000` | — | 14.707 ms | 14.488 ms | 15.778 ms | new |
| `hot-stream/tool-output-hidden-5000000` | — | 6.375 µs | 6.042 µs | 7.375 µs | new |
| `hot-stream/shown-clean-frame` | 126.295 µs | 211.018 µs | 208.643 µs | 225.922 µs | +67.1% |
| `hot-stream/shown-dirty-frame` | 49.863 ms | 43.919 ms | 43.518 ms | 45.214 ms | -11.9% |
| `hot-stream/hidden-clean-frame` | 1.992 µs | 1.824 µs | 1.791 µs | 1.850 µs | -8.4% |
| `hot-stream/hidden-dirty-frame` | 12.750 µs | 4.666 µs | 4.250 µs | 5.583 µs | -63.4% |
| `hot-stream/hidden-first-show-frame` | 49.962 ms | 44.075 ms | 43.641 ms | 46.879 ms | -11.8% |
| `hot-swiper/ten-1mb-rows-first-frame` | 2.920 ms | 2.915 ms | 2.812 ms | 3.071 ms | -0.2% |

## Services ([#404 after](../2026-11-01-hot-paths/after-services.tsv))

| Benchmark | #404 after | Current median | Min | Max | Change |
| --- | ---: | ---: | ---: | ---: | ---: |
| `index/10k-files-two-roots-warm-cache` | 21.422 ms | 21.496 ms | 20.961 ms | 22.568 ms | +0.3% |
| `grep/10k-hits-two-roots-complete-warm-cache` | 1089.458 ms | 969.294 ms | 919.614 ms | 1028.527 ms | -11.0% |
| `grep/first-batch-two-roots-warm-cache` | 922.833 µs | 939.042 µs | 853.209 µs | 1.067 ms | +1.8% |
| `grep/sparse-later-root-first-callback-warm-cache` | 681.456 ms | 607.839 ms | 595.384 ms | 627.645 ms | -10.8% |
| `grep/sparse-later-root-complete-warm-cache` | 1062.129 ms | 952.084 ms | 935.368 ms | 973.055 ms | -10.4% |
| `grep/10k-hits-two-roots-complete-warm-cache-4-workers` | 391.947 ms | 353.621 ms | 346.894 ms | 375.987 ms | -9.8% |
| `grep/first-batch-two-roots-warm-cache-4-workers` | 1.705 ms | 1.708 ms | 1.627 ms | 1.763 ms | +0.2% |
| `grep/sparse-later-root-first-callback-warm-cache-4-workers` | 320.559 ms | 261.421 ms | 222.080 ms | 310.996 ms | -18.4% |
| `grep/sparse-later-root-complete-warm-cache-4-workers` | 360.720 ms | 335.847 ms | 325.525 ms | 354.938 ms | -6.9% |
