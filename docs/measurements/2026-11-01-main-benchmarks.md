# macOS full benchmark baseline, 2026-11-01

Latest fetched `main`: `6c0ef19fc80eabe0927b7ae8de1cfac7facb314b`. Release builds, Rust 1.99.0, Apple M5 Pro (18 logical CPUs, 48 GiB), macOS 27.0.1 (26A434), AC power, configured power mode `0`.

Completed 69 core and 9 service benchmarks, plus the GUI baseline/yes/cat scenarios with 120 input-to-paint samples each. Suites ran sequentially. All benchmark self-checks and the existing result parsers passed.

[Aggregate results and raw TSV/samples](2026-11-01-main-benchmarks.json) · [Temporary benchmark patch](2026-11-01-main-benchmarks.patch)

## Harness corrections

Unmodified main aborts at `crates/aiditor-core/benches/core.rs:103`: Swiper expects `"1/80000 "`, but the count now preserves its previous width and returns `" 1/80000 "`. Fixing those two assertions exposes another obsolete check: finder waits 30 seconds for `1/10000`, while current file prompts show `*/10000`.

With user approval, core ran from a temporary `git archive` copy with only benchmark expectations corrected: Swiper retains width eight; finder distinguishes initial `*/N` from selected `1/N` after typing, while ignoring left padding when waiting for matches. The final total-count assertion still checks the full string. The production files, fixtures, sampling helper and release profile were unchanged. A byte comparison against every tracked checkout file confirmed that only the two benchmark source files differed. The working checkout retains its original benchmark code.

## Comparison with PR #314

Compared with the **after** column in [PR #314](https://github.com/Root-man/aiditor/pull/314), merge `9c261c5f5b583107c253736bbfdaf020f279bd7f`. All 72 baseline names are present; six service measurements are new. Percentages use the rounded values published in that PR. Positive means slower.

These are cross-revision observations, not isolated causes or controlled hardware comparisons: #314 reports macOS but does not supply full host/toolchain metadata. The core fixtures and shared sampling helper match #314; the core API calls now pass additional display arguments. Services gained worker configuration, arrival-order delivery, cancellation changes and one extra sparse-hit line in the fixture, so even shared service names are not byte-identical benchmark definitions.

Largest increases by median ratio:

| Benchmark | #314 after | Current | Ratio |
| --- | ---: | ---: | ---: |
| `syntax-edit-frame/rs/100000-lines-character-undo-pair` | 918.8 µs | 892.180 ms | 971.0× |
| `syntax-edit-frame/tsx/100000-lines-character-undo-pair` | 1.33 ms | 1053.176 ms | 791.9× |
| `revert-apply-frame/rs/100k-lines-one-line` | 646.9 µs | 456.250 ms | 705.3× |
| `syntax-edit-frame/rs/10000-lines-character-undo-pair` | 633.2 µs | 84.841 ms | 134.0× |
| `revert-apply-frame/rs/100k-lines-replace-1000-line-block` | 4.14 ms | 462.184 ms | 111.6× |

The aborted core attempt also completed the syntax-edit cases before reaching the finder failure; its medians are retained in the JSON. The large edit/frame increases appeared in both attempts. No cause was profiled or attributed in this run.

## Complete headless results

Times are per operation. Median/minimum/maximum summarize 21 warmed batches (single operations for prepared cases), not event latency percentiles or confidence intervals. A name ending in `pair` includes both operations. Headless frame construction excludes GPUI shaping and painting. Filesystem measurements use warm caches.

| Benchmark | #314 after | Current median | Minimum | Maximum | Change |
| --- | ---: | ---: | ---: | ---: | ---: |
| `buffer-load/100k-lines` | 582.4 µs | 535.797 µs | 524.193 µs | 724.365 µs | -8.0% |
| `edit-undo/100k-lines` | 129.3 ns | 133.0 ns | 130.3 ns | 141.6 ns | +2.9% |
| `display/50-rows-in-100k-lines` | 57.7 µs | 54.450 µs | 53.686 µs | 60.194 µs | -5.6% |
| `swiper/two-queries-in-100k-lines` | 6.94 ms | 6.777 ms | 6.695 ms | 7.041 ms | -2.3% |
| `terminal-output/64k-batch-of-yes` | 658.7 µs | 623.833 µs | 615.010 µs | 657.628 µs | -5.3% |
| `terminal-output/64k-batch-of-cat` | 363.6 µs | 356.286 µs | 333.173 µs | 381.734 µs | -2.0% |
| `terminal-display/50x120-screen` | 40.7 µs | 65.974 µs | 63.273 µs | 67.326 µs | +62.1% |
| `syntax-load/rs/10000-lines-warm-grammar` | 25.88 ms | 25.960 ms | 25.644 ms | 28.807 ms | +0.3% |
| `syntax-edit-frame/rs/10000-lines-character-undo-pair` | 633.2 µs | 84.841 ms | 84.015 ms | 90.963 ms | +13298.8% |
| `syntax-edit-frame/rs/10000-lines-comment-delimiter-undo-pair` | 14.93 ms | 77.524 ms | 77.003 ms | 81.586 ms | +419.2% |
| `syntax-display/rs/10000-lines-stationary` | 179.6 µs | 186.462 µs | 182.547 µs | 191.796 µs | +3.8% |
| `syntax-display/rs/10000-lines-scroll-pair` | 638.4 µs | 722.214 µs | 706.164 µs | 759.992 µs | +13.1% |
| `syntax-load/rs/100000-lines-warm-grammar` | 270.55 ms | 272.042 ms | 269.347 ms | 292.295 ms | +0.6% |
| `syntax-edit-frame/rs/100000-lines-character-undo-pair` | 918.8 µs | 892.180 ms | 884.967 ms | 904.656 ms | +97002.8% |
| `syntax-edit-frame/rs/100000-lines-comment-delimiter-undo-pair` | 152.10 ms | 826.622 ms | 818.414 ms | 842.016 ms | +443.5% |
| `syntax-display/rs/100000-lines-stationary` | 179.7 µs | 205.678 µs | 188.902 µs | 219.846 µs | +14.5% |
| `syntax-display/rs/100000-lines-scroll-pair` | 649.3 µs | 745.721 µs | 733.714 µs | 802.284 µs | +14.9% |
| `syntax-load/tsx/10000-lines-warm-grammar` | 35.92 ms | 34.678 ms | 34.179 ms | 36.152 ms | -3.5% |
| `syntax-edit-frame/tsx/10000-lines-character-undo-pair` | 1.02 ms | 101.962 ms | 100.199 ms | 108.648 ms | +9896.3% |
| `syntax-edit-frame/tsx/10000-lines-comment-delimiter-undo-pair` | 2.53 ms | 103.389 ms | 102.272 ms | 105.995 ms | +3986.5% |
| `syntax-display/tsx/10000-lines-stationary` | 195.9 µs | 204.876 µs | 198.219 µs | 220.180 µs | +4.6% |
| `syntax-display/tsx/10000-lines-scroll-pair` | 916.6 µs | 1.016 ms | 997.268 µs | 1.036 ms | +10.9% |
| `syntax-load/tsx/100000-lines-warm-grammar` | 351.77 ms | 350.880 ms | 344.528 ms | 362.794 ms | -0.3% |
| `syntax-edit-frame/tsx/100000-lines-character-undo-pair` | 1.33 ms | 1053.176 ms | 1045.035 ms | 1062.563 ms | +79086.2% |
| `syntax-edit-frame/tsx/100000-lines-comment-delimiter-undo-pair` | 15.94 ms | 1076.344 ms | 1071.531 ms | 1096.941 ms | +6652.5% |
| `syntax-display/tsx/100000-lines-stationary` | 232.5 µs | 245.206 µs | 242.985 µs | 253.212 µs | +5.5% |
| `syntax-display/tsx/100000-lines-scroll-pair` | 984.2 µs | 1.115 ms | 1.104 ms | 1.139 ms | +13.3% |
| `syntax-load/js/10000-lines-warm-grammar` | 23.03 ms | 23.147 ms | 22.761 ms | 23.450 ms | +0.5% |
| `syntax-load/ts/10000-lines-warm-grammar` | 17.15 ms | 18.043 ms | 17.408 ms | 18.713 ms | +5.2% |
| `syntax-load/ex/10000-lines-warm-grammar` | 30.52 ms | 43.898 ms | 43.244 ms | 45.427 ms | +43.8% |
| `syntax-load/heex/10000-lines-warm-grammar` | 19.65 ms | 55.348 ms | 54.725 ms | 57.252 ms | +181.7% |
| `syntax-load/yaml/10000-lines-warm-grammar` | 31.47 ms | 33.983 ms | 33.475 ms | 38.607 ms | +8.0% |
| `syntax-load/toml/10000-lines-warm-grammar` | 19.76 ms | 16.902 ms | 16.582 ms | 17.526 ms | -14.5% |
| `syntax-load/md/10000-lines-warm-grammar` | 15.05 ms | 32.421 ms | 31.631 ms | 35.138 ms | +115.4% |
| `syntax-display/js/100kb-single-line` | 217.6 µs | 207.340 µs | 198.277 µs | 227.879 µs | -4.7% |
| `syntax-display/rs/1mb-single-line-at-end` | 6.9 µs | 7.692 µs | 7.412 µs | 8.031 µs | +11.5% |
| `syntax-display/rs/1mb-single-line-wrapped-at-end` | 314.7 µs | 291.434 µs | 288.671 µs | 294.605 µs | -7.4% |
| `finder/10000-paths-open-close` | 455.0 µs | 515.164 µs | 503.083 µs | 525.930 µs | +13.2% |
| `finder/10000-paths-narrow-broaden-pair` | 595.5 µs | 691.924 µs | 661.901 µs | 797.513 µs | +16.2% |
| `finder/100000-paths-open-close` | 4.57 ms | 5.256 ms | 5.160 ms | 5.957 ms | +15.0% |
| `finder/100000-paths-narrow-broaden-pair` | 2.88 ms | 3.373 ms | 3.090 ms | 3.671 ms | +17.1% |
| `revert-diff/txt/100k-lines-one-line` | 12.20 ms | 11.884 ms | 11.656 ms | 13.145 ms | -2.6% |
| `revert-apply-frame/txt/100k-lines-one-line` | 117.4 µs | 132.416 µs | 107.833 µs | 150.042 µs | +12.8% |
| `revert-diff/txt/100k-lines-scattered` | 18.09 ms | 16.890 ms | 16.718 ms | 17.208 ms | -6.6% |
| `revert-apply-frame/txt/100k-lines-scattered` | 675.2 µs | 617.375 µs | 573.375 µs | 697.625 µs | -8.6% |
| `revert-diff/txt/100k-lines-replace-1000-line-block` | 14.07 ms | 13.851 ms | 13.547 ms | 14.190 ms | -1.6% |
| `revert-apply-frame/txt/100k-lines-replace-1000-line-block` | 147.6 µs | 152.708 µs | 138.708 µs | 179.250 µs | +3.5% |
| `revert-diff/rs/100k-lines-one-line` | 12.16 ms | 11.824 ms | 11.578 ms | 12.474 ms | -2.8% |
| `revert-apply-frame/rs/100k-lines-one-line` | 646.9 µs | 456.250 ms | 452.555 ms | 467.634 ms | +70428.6% |
| `revert-diff/rs/100k-lines-scattered` | 17.15 ms | 16.951 ms | 16.707 ms | 17.240 ms | -1.2% |
| `revert-apply-frame/rs/100k-lines-scattered` | 34.59 ms | 487.143 ms | 484.388 ms | 496.400 ms | +1308.3% |
| `revert-diff/rs/100k-lines-replace-1000-line-block` | 13.78 ms | 13.700 ms | 13.410 ms | 14.065 ms | -0.6% |
| `revert-apply-frame/rs/100k-lines-replace-1000-line-block` | 4.14 ms | 462.184 ms | 458.397 ms | 466.680 ms | +11063.9% |
| `terminal-copy/plain/10k-lines-enter-exit-pair` | 9.86 ms | 25.077 ms | 24.900 ms | 26.383 ms | +154.3% |
| `terminal-copy/plain/10k-lines-display` | 50.4 µs | 50.362 µs | 49.788 µs | 53.716 µs | -0.1% |
| `terminal-copy/plain/10k-lines-scroll-pair` | 141.5 µs | 146.885 µs | 145.410 µs | 149.149 µs | +3.8% |
| `terminal-copy/colored/10k-lines-enter-exit-pair` | 52.81 ms | 68.741 ms | 68.257 ms | 70.473 ms | +30.2% |
| `terminal-copy/colored/10k-lines-display` | 212.6 µs | 203.284 µs | 200.785 µs | 206.393 µs | -4.4% |
| `terminal-copy/colored/10k-lines-scroll-pair` | 3.20 ms | 3.149 ms | 3.120 ms | 3.212 ms | -1.6% |
| `terminal-output/ansi-tui-osc-trace` | 411.7 µs | 434.921 µs | 424.592 µs | 448.319 µs | +5.6% |
| `terminal-display/ansi-unicode-50x120` | 55.5 µs | 74.960 µs | 71.813 µs | 77.805 µs | +35.1% |
| `terminal-resize/10k-lines-120-to-80-columns` | 2.12 ms | 2.309 ms | 1.566 ms | 2.450 ms | +8.9% |
| `frame/1-panes-shared-rust` | 131.5 µs | 144.290 µs | 140.696 µs | 156.147 µs | +9.7% |
| `frame/2-panes-shared-rust` | 269.7 µs | 282.172 µs | 278.561 µs | 288.846 µs | +4.6% |
| `frame/4-panes-shared-rust` | 522.2 µs | 561.536 µs | 554.003 µs | 576.831 µs | +7.5% |
| `frame/1-panes-distinct-rust` | 127.7 µs | 140.796 µs | 139.035 µs | 147.900 µs | +10.3% |
| `frame/2-panes-distinct-rust` | 256.3 µs | 284.368 µs | 278.159 µs | 291.773 µs | +11.0% |
| `frame/4-panes-distinct-rust` | 515.6 µs | 570.172 µs | 558.624 µs | 596.992 µs | +10.6% |
| `session-render/5000-line-styled-message` | 47.28 ms | 48.924 ms | 48.224 ms | 52.127 ms | +3.5% |
| `index/10k-files-two-roots-warm-cache` | 26.14 ms | 22.987 ms | 22.556 ms | 23.418 ms | -12.1% |
| `grep/10k-hits-two-roots-complete-warm-cache` | 970.57 ms | 1143.684 ms | 1056.631 ms | 1166.135 ms | +17.8% |
| `grep/first-batch-two-roots-warm-cache` | 1.05 ms | 988.083 µs | 949.125 µs | 1.193 ms | -5.9% |
| `grep/sparse-later-root-first-callback-warm-cache` | — | 692.405 ms | 684.227 ms | 707.596 ms | new |
| `grep/sparse-later-root-complete-warm-cache` | — | 1082.027 ms | 1072.673 ms | 1099.829 ms | new |
| `grep/10k-hits-two-roots-complete-warm-cache-4-workers` | — | 389.121 ms | 375.872 ms | 429.671 ms | new |
| `grep/first-batch-two-roots-warm-cache-4-workers` | — | 1.764 ms | 1.675 ms | 1.814 ms | new |
| `grep/sparse-later-root-first-callback-warm-cache-4-workers` | — | 307.476 ms | 284.454 ms | 343.493 ms | new |
| `grep/sparse-later-root-complete-warm-cache-4-workers` | — | 370.569 ms | 358.931 ms | 392.144 ms | new |

## GUI input during terminal floods

Built-in Color LCD, reported 120 Hz, 3456×2234 physical / 1728×1117 logical resolution. The isolated benchmark uses `WindowOptions::default()`; GPUI 0.2.2 specifies a default size of 1536×864 logical pixels on this display. Actual window bounds were not separately sampled. The app exits after measurement and kills the flood processes.

| Scenario / metric | Samples | p50 | p95 | p99 | Maximum |
| --- | ---: | ---: | ---: | ---: | ---: |
| baseline / input-to-paint | 120 | 24.911 ms | 26.057 ms | 26.674 ms | 29.893 ms |
| baseline / paint-gap | 119 | 74.935 ms | 76.156 ms | 76.687 ms | 79.915 ms |
| yes / input-to-paint | 120 | 16.659 ms | 17.079 ms | 17.655 ms | 21.313 ms |
| yes / paint-gap | 962 | 8.332 ms | 18.889 ms | 19.139 ms | 20.563 ms |
| cat / input-to-paint | 120 | 16.674 ms | 17.109 ms | 17.544 ms | 17.970 ms |
| cat / paint-gap | 963 | 8.333 ms | 18.999 ms | 19.195 ms | 19.998 ms |

Processed terminal bytes during measurement:

- `baseline`: 0
- `yes`: 165,934,238
- `cat`: 191,903,999

GUI timings measure scheduled synthetic input through the matching frame paint callback. They exclude OS keyboard delivery, GPU completion and display scanout. Paint gaps include idle time and the input cadence; they are not GPU frame durations. PR #314 did not run this suite, so there is no GUI baseline comparison.

## Reproduce

```sh
python3 scripts/bench_local.py --suite services --output /tmp/services.tsv
# In a temporary source copy with the saved benchmark patch applied:
BENCH_OUTPUT=/tmp/core.tsv cargo bench --locked -p aiditor-core --bench core
# Back in the unchanged main checkout:
python3 scripts/bench_local.py --suite ui --output /tmp/ui.tsv
```

Not checked: Linux, ordinary tests/clippy, physical keyboard-to-display latency, or regression root causes. No refactor scan, production edit, commit, or GitHub write was performed.
