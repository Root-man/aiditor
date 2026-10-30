Scope: The whole project at `d3c9565` (`Performance benchmarks (#264)`), refreshing the scan made before #108 and M2; recorded on 2026-10-25, with the reported macOS/Linux grep comparison investigated through source inspection, targeted macOS measurements, profiling, and a temporary throughput prototype.

What this code does: aiditor is a keyboard-first Rust editor, with editor state in `aiditor-core`, filesystem/process work in `aiditor-services`, and a GPUI frontend in `aiditor`.

| # | Opportunity | Category | Where | Impact | Complexity | Fix |
|---|---|---|---|---|---|---|
| 1 | Normal shell exit leaves PTY handles and input threads alive until application shutdown. | Bug | [core/terminal.rs:716](../crates/aiditor-core/src/terminal.rs#L716), [services/terminal.rs:134](../crates/aiditor-services/src/terminal.rs#L134) | high | med | Send the existing terminal cleanup effect on exit and failed spawn before forgetting the terminal; verify resource release. |
| 2 | Recovery panics when the visited disk file has become binary/read-only. | Bug | [core/autosave.rs:60](../crates/aiditor-core/src/autosave.rs#L60) | high | med | Use the existing undoable `Buffer::replace` operation, which supports replacing read-only content, and cover recovery into a refused disk file. |
| 3 | Long-line redraw scans the hidden prefix and builds highlights outside the horizontal viewport. | Performance | [core/display.rs:154](../crates/aiditor-core/src/display.rs#L154), [core/display.rs:234](../crates/aiditor-core/src/display.rs#L234), [core/syntax.rs:209](../crates/aiditor-core/src/syntax.rs#L209) | high | med | Skip hidden prefixes directly where folds permit; limit highlight painting and conversion to the displayed ranges while retaining syntax context. |
| 4 | Terminal copy mode repeatedly counts an increasingly long soft-wrapped logical line. | Performance | [core/terminal.rs:296](../crates/aiditor-core/src/terminal.rs#L296) | high | med | Track the logical line's character count as rows are appended, resetting at a hard newline; preserve Unicode positions and colours. |
| 5 | Serial file opens dominate the roughly 960 ms complete grep on this Mac: opening alone takes about 844 ms. | Performance | [services/grep.rs:88](../crates/aiditor-services/src/grep.rs#L88), [services/files.rs:212](../crates/aiditor-services/src/files.rs#L212), [services bench:59](../crates/aiditor-services/benches/services.rs#L59) | high | high | Deferred to M5 in [#265](https://github.com/Root-man/aiditor/issues/265): add parallel file searches controlled by `grep_workers` in `settings.toml`, default `1`, preserving order, streaming, cancellation, and bounded queues. The prototype measured 439 ms with 4 workers and 380 ms with 8. |
| 6 | A failed replacement recovery write can be followed by deletion of the old recovery file. | Risk | [core/autosave.rs:25](../crates/aiditor-core/src/autosave.rs#L25), [core/autosave.rs:73](../crates/aiditor-core/src/autosave.rs#L73), [services/recovery.rs:21](../crates/aiditor-services/src/recovery.rs#L21) | high | high | Acknowledge successful persistence before marking a buffer tick auto-saved or deleting its previous recovery copy; retain the old copy and allow retry after failure. |
| 7 | `char_idx` walks the whole line just to inspect its final character. | Performance | [core/buffer.rs:639](../crates/aiditor-core/src/buffer.rs#L639) | low | low | Read the final character by index, handling an empty slice, instead of calling `chars().last()`. |

One smaller opportunity omitted: display construction collects all copy-mode properties before filtering to the viewport. File size alone was not treated as a finding.

#1: The core removes the terminal mapping before buffer cleanup, suppressing the cleanup effect. The service removes its retained handle only on `Kill` or `kill_all`; the input thread waits on a sender held by that handle. An earlier probe on this revision ran twelve `/bin/sh -c 'exit 0'` terminals: open descriptors rose from 9 to 33 and returned to 9 after `kill_all`.

#2: An earlier probe offered valid recovered text for a disk file containing bytes `[0, 1, 2]`. Opening it produced a read-only buffer; recovery's `insert(...).expect(...)` panicked with `ReadOnly`, leaving the buffer empty. `Buffer::replace` already has a test for an undoable replacement past read-only.

#3: Earlier optimized core-display probes, with parsing outside the timing, took about 41–44 ms per frame near the end of a 1 MB single line. The source walks each hidden prefix character; syntax also paints an array covering the requested whole lines. These are core display timings, not GPUI frame measurements.

#4: Earlier optimized copy-text probes with 120-column rows took about 5.5 ms for a logical line spanning 1,000 rows and 216–221 ms for 10,000 rows. Recounting the growing prefix on every wrapped row makes that part quadratic.

#6: An earlier probe forced the replacement write to fail by occupying its temporary-file path with a directory. The recovery worker logged the failure and processed the following deletion, leaving no old or replacement recovery copy. The recovered buffer still exists in memory, but a subsequent crash can lose its only remaining copy.

## Grep investigation (#5)

**Status:** Built in [#265 — Parallel project grep with configurable workers](https://github.com/Root-man/aiditor/issues/265): the opt-in `grep_workers` setting (default `1`) searches files on that many workers with ordered streaming delivery; see [Performance benchmarks](performance.md#grep-workers) for production measurements. The rest of this section records the original investigation.

**Measured conclusion:** opening files is the dominant bottleneck on this Mac. The original complete-search scenario reproduced at **965.7 ms median**. Instrumentation attributed **843.6 ms** to file opens out of a **959.8 ms** search; sampling located **91.6% of samples inside `__open`**. Reading bytes, regex work, traversal, and result handling are much smaller. This identifies the expensive operation; it does not identify which kernel component or security product makes that operation expensive.

A temporary parallel-file prototype reduced complete-search time from **933.4 ms with one worker to 379.6 ms with eight workers**, a **2.46× improvement**. Every ordered `Hit` matched production on this fixture. This supports bounded parallel file search as a refactor, but neither demonstrates a 10× improvement nor validates the production streaming/cancellation design.

### Environment and measurement boundaries

Measured on 2026-10-25 at `d3c9565`: Apple M5 Pro, 18 logical CPUs, ARM64, macOS 27.0.1 (26A434), Rust 1.99.0 (`b940084d7`), optimized release build with the repository's locked dependencies. Linux's approximately 95 ms remains the user's supplied comparison; no Linux execution or matching hardware/compiler comparison was performed here.

The temporary harness copied [the service benchmark](../crates/aiditor-services/benches/services.rs), retained its fixture and validation, and removed the **timed index and first-batch scenarios**. Production runs call the existing `grep::search`. All reported benchmark medians use [the existing helper](../benches/support.rs): warm-up followed by 21 batches. No unrelated benchmark was run. Temporary harnesses were built offline using installed dependencies; nothing was installed.

The fixture has two roots with 5,000 source files each, in 100 source directories total. Every source file is 1,063 bytes, with one Unicode hit after 50 ordinary lines. Including the two searched `.gitignore` files, the instrumented query read **10,630,018 bytes through 30,005 read calls**. Fixture construction and initial validation are outside timing, and the same files are reused across all repetitions within each invocation.

The timed search includes traversal, ignore rules, regex construction, file access, matching, result allocation, clock checks, and batch validation. It excludes prompt debounce, Tokio channel/task scheduling, and GUI rendering. The Python wrapper is outside the Rust operation timer.

### Reproduction and stage attribution

| Check | Median | Evidence / interpretation |
|---|---:|---|
| User's original macOS result | 999.674 ms | Supplied minimum 932.436 ms, maximum 1191.942 ms; Linux reported approximately 95 ms. |
| Isolated production scenario | 965.662 ms | Minimum 926.228 ms, maximum 1096.506 ms; reproduces the reported delay. |
| Production outside the execution sandbox, with sampling | 964.935 ms | Similar median; profiler overhead affects individual samples, so this is a control rather than the primary timing. |
| Production with fixture under `/private/tmp` | 912.757 ms | Minimum 879.069 ms, maximum 1014.202 ms; changing the default `/var/folders/...` location does not remove the large delay. |
| Instrumented production search | 959.906 ms | Similar to the uninstrumented baseline; component timings below include instrumentation overhead. |

The instrumented version kept the production walk and sink, splitting `search_path` into `File::open`, reader search, and explicit close. Its reader wrapper timed each read. With this configuration, locked `grep-searcher` 0.1.17 already uses reader search: mmap is disabled by default on both platforms. That dependency also declines mmap explicitly on macOS, so enabling it is not a fix here.

| Stage | Median per complete query | Interpretation |
|---|---:|---|
| File opens | 843.592 ms | About 88% of the instrumented query; roughly 84 µs per searched file. |
| File closes | 55.410 ms | About 6%. |
| Directory traversal and ignore processing | 27.838 ms | About 3%; parallelizing only the walker misses the dominant cost. |
| Search, including reads and result callbacks | 30.252 ms | About 3%. |
| Reads, included in the search row | 21.742 ms | Do not add this row again; only about 8.5 ms of the search row remains for decoding, matching, callbacks, and associated overhead. |

These are component medians over the 21 measured queries; they need not sum exactly to the median total. The uninstrumented eight-second `sample` profile recorded 5,370 main-thread samples: **4,920 in `__open` (91.6%)**, **201 in `close` (3.7%)**, and **72 in `read` (1.3%)**. The dominant call chain was `search_with_clock → Searcher::search_path → File::open → open → __open`. Sampling proportions are statistical evidence, not exact duration accounting.

### Security-software attribution

Read-only checks found active **Microsoft Defender Endpoint Security** and **Elastic Endpoint Security** extensions. Defender reported real-time protection enabled. This makes endpoint-security overhead a concrete hypothesis, but neither extension presence nor the `__open` stack attributes the measured latency to a particular product. There is no evidence specifically implicating XProtect.

Defender's scan-statistics query returned `No Data`. [Microsoft's diagnostic guide](https://learn.microsoft.com/en-ie/defender-endpoint/mac-support-perf) describes separately enabling scan statistics and notes their collection limits; no Defender diagnostic configuration, exclusion, or protection setting was changed. A deeper Instruments System Trace attempt timed out and produced no usable attribution; the successful application sampling remains the profiling evidence.

The repeated warmed measurements and the alternate temporary directory rule out neither ongoing security checks nor ordinary filesystem overhead. They do show that another ordinary invocation, or moving the fixture to `/private/tmp`, does not make this a Linux-speed search. Attribution to Defender versus Elastic versus another kernel/filesystem cost would require additional targeted telemetry or a separately approved controlled security-policy comparison. No protection was disabled.

### Parallel-search experiment

The diagnostic prototype enumerated the same ordered file set, distributed file searches across workers, reused one matcher/searcher per worker, and restored file order before delivering the results. Its single-worker control uses the same prototype path. Before timing each worker count, all fields of all 10,000 ordered hits were compared with production and matched.

| Workers | Median | Minimum | Maximum | Speedup over prototype with 1 worker |
|---|---:|---:|---:|---:|
| 1 | 933.371 ms | 909.003 ms | 949.731 ms | 1.00× |
| 2 | 565.392 ms | 542.680 ms | 650.487 ms | 1.65× |
| 4 | 439.049 ms | 372.427 ms | 506.944 ms | 2.13× |
| 8 | 379.555 ms | 359.662 ms | 403.987 ms | 2.46× |

This experiment measures **complete-search throughput only**. It buffers results until completion and does not implement production streaming, cancellation, or early global-limit termination. It is not a replacement implementation. The measurements were sequential on one machine, not randomized cross-platform trials; their large improvement supports the direction, while exact ratios can vary. The diminishing return above four workers also means worker count should remain bounded.

### Proposed change and acceptance criteria

Built in [#265](https://github.com/Root-man/aiditor/issues/265) as proposed below; the criteria are kept as the record of what it was built against. Expose the worker count as the positive integer `grep_workers` in `settings.toml`, with bundled default `1`. One worker preserves serial searching; a higher configured count bounds concurrent file searches. Use the existing settings validation and reload path, with changes applying to the next search and an active search retaining its captured count.

Keep the existing ordered, ignore-aware walk initially and overlap file searches within that configured bound. Retain a searcher per worker. A coordinator should own the public `cancelled` and `send` callbacks and deliver incremental chunks in current root/walk/line order. Bound in-flight files and result queues, and stop outstanding work when cancelled or when the ordered global cap is reached. The setting crosses the core configuration and service boundary, so this row's complexity is now high.

Locked `ignore` 0.4.33 ignores `sort_by_file_name` when using `build_parallel`, so a direct builder substitution would break [the specified order](editor.md#subsystems) and its test (`lists_roots_in_order_and_files_in_path_order` in `crates/aiditor-services/src/grep.rs`). The proposal needs parallel **file searching**, not just a parallel directory walk. Keeping every file open or caching all file contents would introduce descriptor limits or invalidation responsibilities; neither is needed for this refactor.

- **Given** `grep_workers` is omitted or set to `1`, **when** project grep runs, **then** it uses one file-search worker and preserves serial behavior.
- **Given** a valid `grep_workers = N` greater than one and enough files, **when** project grep runs, **then** file searches overlap with at most N workers and deliver the same ordered hits as a one-worker search.
- **Given** the worker setting changes and is reloaded, **when** the next search starts, **then** it uses the new count; an already-running search keeps its captured count.
- **Given** a zero, negative, or non-integer worker setting, **when** settings load or reload, **then** the existing error path reports it and retains the last valid settings, or bundled defaults at startup.
- **Given** files in several roots whose searches finish out of order, **when** results are delivered, **then** their root, walk, and line order matches today's service.
- **Given** one large matching file, **when** it is still being searched, **then** hits stream at the existing approximate 50 ms checks rather than waiting for file completion.
- **Given** a superseding query or cancellation, **when** workers and the coordinator observe it at their file/match/send checks, **then** they stop work and the coordinator emits no further results for that search.
- **Given** more than 10,000 hits, **when** the default search reaches its cap, **then** exactly the first 10,000 ordered hits and one final completion are delivered, and unnecessary work stops.
- [x] Preserve ignore rules, binary handling, Unicode columns, unreadable-file handling, and the existing callback API for production, headless, benchmark, and test callers (the cancel check is now `Sync`, as workers call it).
- [x] Bound worker count, queued work, and buffered results; cover cancellation while queues are full and an earlier file is slow.
- [x] Validate production streaming and cancellation through behavioral tests; the throughput prototype does not establish these properties.
- [ ] Repeat complete-search measurements after implementation; separately obtain approval before running other benchmark scenarios, including first-batch latency. Validate on Linux before treating this as a cross-platform improvement.

Temporary diagnostic artifacts are under `/private/tmp/aiditor-grep-investigation-20261025/`: `baseline.rs`, `src/main.rs`, `src/parallel.rs`, `src/instrumented.rs`, `src/stats.rs`, `baseline.sample.txt`, per-run TSV/log files, and `summary.json`. They are investigation artifacts, not repository implementation code. The table and component measurements above preserve the results if that temporary directory is later removed.

## Verification and limits

The earlier scan on this revision passed 570 Rust core/service tests with one ignored, plus 21 Python benchmark-reporting tests. Those tests and the earlier non-grep probes were not rerun for this investigation. The new work reproduced the authorized grep scenario, cross-checked application sampling against per-stage timing, changed fixture location, and compared the parallel prototype against every ordered production hit. The deferred task is recorded in `docs/roadmap.yaml` and GitHub issue #265. Repository implementation code is unchanged.

Start with: **1 and 2**, because existing cleanup/replacement operations address concrete bugs with limited changes. **6** also deserves early attention because failed recovery persistence can remove the remaining on-disk copy. **5 is deferred to M5 in [#265](https://github.com/Root-man/aiditor/issues/265)**, with configurable parallelism defaulting to one worker.

Not checked: GUI runtime behavior, Linux execution, other benchmark scenarios, or production parallel-search streaming/cancellation; the optional `ponytail-audit` skill was unavailable. File-open latency is measured, but attribution to a specific security product or kernel component remains unproven.
