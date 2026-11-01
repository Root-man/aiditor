# Editor hot paths and acknowledged auto-save, 2026-11-01

Matched release measurements on Apple M5 Pro (18 logical CPUs, 48 GiB), macOS 27.0.1 (26A434), Rust 1.99.0. The base is `6c0ef19fc80eabe0927b7ae8de1cfac7facb314b`. Baseline production behavior was unchanged. The benchmark assertion repairs, new focused target and its manifest entry were in place before the retained baseline; test-only additions did not alter release behavior. The same benchmark sources and sampling helper were used after implementation.

The repaired full core suite (69 cases), focused suite (14) and service suite (9) ran sequentially for each side. All self-checks and existing TSV parsers passed, with matching names in every before/after pair. The exploratory overlapping runs were discarded; only sequential runs are retained here. The existing helper records median/minimum/maximum of 21 batches, or 21 prepared single-operation samples after three warm-ups. TSV files contain those batch summaries, not individual event samples or confidence intervals.

## Provenance and retained evidence

- [Baseline metadata and supplied-input hashes](baseline-metadata.json), [implementation metadata and source hashes](after-metadata.json), [display and power metadata](display-metadata.json).
- Raw core summaries: [baseline](baseline-core.tsv), [after](after-core.tsv).
- Raw focused summaries: [baseline](baseline-focused.tsv), [after](after-focused.tsv).
- Raw service summaries: [baseline](baseline-services.tsv), [after](after-services.tsv).
- [All 92 comparisons](comparisons.json), [scope diagnostic](scope-reuse.log), [preview component diagnostic](preview-work.log).
- GUI events and parser reports: [after TSV](after-ui.tsv), [report](after-ui.md), [repeat TSV](after-ui-repeat.tsv), [repeat report](after-ui-repeat.md).
- [Scenario and structural validation](validation.md). Linux execution is pending the draft PR's CI; all local results below are macOS results.

The three supplied `2026-11-01-main-benchmarks` files remain byte-for-byte unchanged. Their recorded hashes were checked again after measurement. Those supplied GUI samples supply the earlier GUI comparison; the new sequential baseline supplies every headless comparison here. No cross-host PR #314 ratios are used in this report.

## Focused core measurements

Times below are median ms per operation. These measure core work, excluding GPUI shaping and painting.

| Case | Baseline ms | After ms |
| --- | ---: | ---: |
| `hot-scopes/10000-lines-first-highlight-after-edit` | 43.818 | 1.142 |
| `hot-scopes/10000-lines-edit-highlight-undo-pair` | 87.538 | 2.157 |
| `hot-scopes/10000-lines-cached-frame` | 0.194 | 0.194 |
| `hot-scopes/100000-lines-first-highlight-after-edit` | 462.244 | 7.018 |
| `hot-scopes/100000-lines-edit-highlight-undo-pair` | 920.406 | 13.558 |
| `hot-scopes/100000-lines-cached-frame` | 0.199 | 0.201 |
| `hot-preview/100000-matches-stationary` | 12.612 | 7.827 |
| `hot-preview/1000000-matches-stationary` | 136.158 | 74.576 |
| `hot-stream/shown-clean-frame` | 0.126 | 0.126 |
| `hot-stream/shown-dirty-frame` | 49.818 | 49.863 |
| `hot-stream/hidden-clean-frame` | 0.002 | 0.002 |
| `hot-stream/hidden-dirty-frame` | 49.469 | 0.013 |
| `hot-stream/hidden-first-show-frame` | 0.160 | 49.962 |
| `hot-swiper/ten-1mb-rows-first-frame` | 38.114 | 2.920 |

The scoped fixture contains independent Rust functions with `let café`, grouped 100 per module. The first-highlight case times the first frame after the prepared edit, including deferred reparse; cached-frame timing uses the already highlighted view. The edit/undo pair includes two frames. Dense previews search `x` on a complete 100,000- or 1,000,000-character line through the normal wrapped viewport. Stream cases append ` next` to a roughly 5,000-line styled reply; clean frames, dirty frames and the first frame after showing a hidden dirty buffer are separate operations. The row case displays ten Rust comment lines containing one million `x` characters each.

Hidden formatting is deferred, not eliminated: shown dirty frames remain about 49.9 ms, and first-show work is now about 50.0 ms. Before the change the hidden frame paid that work and first show was clean. The new hidden dirty frame retains its dirty state and avoids streamed-message parsing. Tool-call rendering and block finalization remain unconditional.

## Existing-suite comparisons and remaining syntax costs

| Existing case | Baseline ms | After ms |
| --- | ---: | ---: |
| `syntax-edit-frame/rs/100000-lines-character-undo-pair` | 898.621 | 14.172 |
| `syntax-edit-frame/tsx/100000-lines-character-undo-pair` | 1058.067 | 4.047 |
| `revert-apply-frame/rs/100k-lines-one-line` | 460.344 | 69.780 |
| `revert-apply-frame/rs/100k-lines-scattered` | 491.305 | 101.619 |

Across the 69 existing core cases, the largest median increase was 9.6% (`syntax-display/tsx/10000-lines-scroll-pair`, 1.006 to 1.102 ms). Across the nine service cases it was 7.8% (four-worker sparse first callback, 297.461 to 320.559 ms). Those smaller differences are retained rather than treated as stable improvements or isolated causes. No timing threshold was added.

The release scope diagnostic highlights 50 lines around the middle of the 10,000/100,000-line fixture. It rebuilt one scope at each size, with 101 scope analyses built initially for the requested view and inherited context. At 100,000 lines it recorded 0.223 ms reparse, 7.084 ms first highlight and 0.001 ms cached highlight. This demonstrates reuse of unrelated scopes; it is not constant-time editing. Cached fragment indexing and root environment collection can still walk existing metadata. Large shared binding environments, changed parse structure, removed/reparented scopes, and changed injection ranges/languages can cause broader rebuilds. Initial parsing and analysis also remain full-source work where needed.

## Preview bounds and remaining whole-line work

The display first records actual source positions, including scrolling, wrapping and folds, then asks each source line's search once for its displayed intervals. The search retains clipped nonempty intersections and their original first-match identity. Only retained boundaries are converted to character positions. Full-line anchors, ordered words, exclusions and smart case still run against complete input. Tests retain exactly 15 spans for two intervals totaling 15 bytes with both 1,000 and 1,000,000 line matches; a hidden first match does not promote a later one.

The separate release diagnostic uses five samples, reports their median, and requests 120 displayed bytes. On one million ASCII characters it measured 39 µs to copy the input line, 89 µs for a full-line exclusion scan, 88 µs for bounded matching including that exclusion, and 5 µs to convert the 120 retained spans. These are separate component measurements, not additive exclusive profiler samples.

One call to the existing word-wrap row-layout routine at 120 columns took 36.3 ms at one million characters, versus 5.8 ms at 100,000. The frame path calls row layout for scrolling and display origin. That measured remaining full-line layout work is consistent with the still line-length-dependent 74.6 ms dense-preview frame; it does not establish exclusive attribution of every millisecond. The optimization bounds search storage/conversion, not all line layout or global regex work. Row-layout changes are outside these six fixes.

## Swiper presentation bounds

The column budget is `(frame width - 2 × 12px) / cell width`, clamped at zero. Only the prefix and source slice that fit are allocated and painted. Navigation uses the canonical candidate, so a match beyond the clipped edge and even a prefix-only/empty presentation still reach the full source target. UTF-8 ranges use the same bounded slice as the text; source snapshot context and the current palette are retained.

Core checks cover zero/narrow widths, resize and cell-width changes, Unicode, full-line matches beyond the edge, offscreen locals, multiline/injected syntax, source edits/reverts and theme reload. The GPUI check shapes ten long rows at four budgets (10, 2, 0 and 80 columns), verifies run lengths and selected syntax backgrounds, and preserves the existing selected row background. That is 40 shaped row checks, not a GUI latency benchmark. The 2.920 ms row measurement above is core frame construction only.

## GUI input-to-paint observations

Built-in Liquid Retina XDR display, 3456×2234 physical / 1728×1117 logical, reported 120 Hz, AC power. The benchmark uses GPUI's default window options; actual window bounds were not separately sampled. Both new GUI runs used the existing baseline/yes/cat scenarios sequentially, 120 inputs per scenario, with the same default settings and temporary PTY fixtures. They ran separately from headless measurements and other builds.

| Scenario / run | Samples | p50 ms | p95 ms | p99 ms | Max ms |
| --- | ---: | ---: | ---: | ---: | ---: |
| baseline / supplied baseline | 120 | 24.911 | 26.057 | 26.674 | 29.893 |
| baseline / after | 120 | 23.708 | 27.958 | 28.631 | 28.713 |
| baseline / after repeat | 120 | 24.332 | 27.611 | 28.418 | 29.569 |
| yes / supplied baseline | 120 | 16.659 | 17.079 | 17.655 | 21.313 |
| yes / after | 120 | 8.374 | 16.738 | 16.920 | 17.008 |
| yes / after repeat | 120 | 8.402 | 16.767 | 16.953 | 17.233 |
| cat / supplied baseline | 120 | 16.674 | 17.109 | 17.544 | 17.970 |
| cat / after | 120 | 8.396 | 16.824 | 16.895 | 26.516 |
| cat / after repeat | 120 | 9.016 | 16.906 | 17.436 | 22.977 |

Cat's maximum was higher than the supplied baseline in both new runs, while its p95/p99 were at or below that baseline. There were 1/120 and 1/120 cat samples above 20 ms. The repeat retains the tail variation; neither run isolates its cause or establishes that these source changes caused it. The headless paths show no corresponding large median regression. Both complete raw event sets are retained, with no outlier removal.

These timestamps cover scheduled synthetic input through the matching frame's paint callback: scheduling, core work, shaping and paint submission. They exclude OS keyboard delivery, GPU completion and display scanout. Paint gaps include the input cadence and idle time and are not GPU frame durations. GUI percentiles must not be compared with headless batch averages.
