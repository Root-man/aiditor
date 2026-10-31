# Performance benchmarks

## Run locally

On macOS or Linux, using Python 3 and the Rust version in `.tool-versions`:

```sh
python3 scripts/bench_local.py
python3 scripts/bench_local.py --suite core
python3 scripts/bench_local.py --suite services
python3 scripts/bench_local.py --output /tmp/aiditor-benchmarks.tsv
```

The default runs both headless suites in release mode and prints median, minimum and maximum times per operation. Cargo diagnostics remain on stderr. Generated files live in a temporary directory and are removed afterwards. No GitHub credentials, additional Python packages or hosted workflow are needed. `--output` retains the combined raw TSV accepted by the CI reporting helper.

Direct Cargo invocations remain available:

```sh
cargo bench --locked -p aiditor-core --bench core
cargo bench --locked -p aiditor-services --bench services
```

Set `BENCH_OUTPUT` to a separate absolute TSV path for each invocation to retain its results. Ordinary `cargo test` does not run these targets; `cargo clippy --all-targets` compiles them on Linux and macOS.

## Core workloads

Fixtures are deterministic and include Unicode. Setup is outside the timed operation unless specified below. `pair` in a name means the time covers both operations, not one half. Rendering here means building the core display model, without GPUI shaping or painting.

| Family | One measured operation |
| --- | --- |
| `buffer-load/100k-lines` | Create a plain-text buffer and Rope, then drop it |
| `edit-undo/100k-lines` | Insert one Unicode character at line 50,000, then undo |
| `display/50-rows-in-100k-lines` | Build and drop 50 visible plain-text lines |
| `swiper/two-queries-in-100k-lines` | Replace the query with `needle`, then `needle !skip`, including filtering and preview; opening is outside the loop |
| `terminal-output/64k-batch-*` | Deliver one approximately 64 KB `yes` or `cat` batch through the core |
| `terminal-display/50x120-screen` | Build runs for a plain terminal screen |
| `syntax-load/*` | Create and drop a highlighted buffer: 10k lines for Rust, TSX, JS, TS, Elixir, HEEx, YAML, TOML and Markdown; also 100k for Rust/TSX |
| `syntax-edit-frame/*` | Insert a space or `/*` at the middle of a Rust/TSX buffer, build a frame, dispatch undo through the key handler, build another frame; 10k/100k lines |
| `syntax-display/*-stationary` | Build a 50-row frame in an already parsed Rust/TSX buffer |
| `syntax-display/*-scroll-pair` | Scroll forward and back by a page, building both frames |
| `syntax-display/js/100kb-single-line` | Display a 100 KB JS string literal through a 120-column viewport, highlighting only the shown columns |
| `syntax-display/rs/1mb-single-line-at-end` | Display the end of a 1 MB Rust string literal, point at the line's end, so the hidden columns before the viewport are skipped |
| `finder/*-open-close` | Open and settle the project finder, then close it, with 10k/100k pre-indexed paths across two roots |
| `finder/*-narrow-broaden-pair` | Narrow to two matching paths, then broaden to all paths, waiting for results and completion polling to settle each time |
| `revert-diff/*` | Compute line edits between two 100k-line strings: one line, 1,000 scattered lines, or a contiguous 1,000-line replacement |
| `revert-apply-frame/*` | Deliver a prepared revert to a fresh clean plain/Rust buffer and build its next frame, including reparse; preparing the buffer and diff is outside timing |
| `terminal-copy/*-enter-exit-pair` | Enter and leave copy mode with 10k lines of plain or coloured scrollback, including text and property creation/removal |
| `terminal-copy/*-display`, `*-scroll-pair` | Display the copy buffer or scroll forward/back with frames |
| `terminal-output/ansi-tui-osc-trace` | Feed a fixed trace with colour, cursor movement, Unicode, alternate screens, OSC 7 and OSC 133; clearing history is included so marks remain active across repetitions |
| `terminal-display/ansi-unicode-50x120` | Build runs for a densely styled, Unicode terminal screen |
| `terminal-resize/*` | Resize a fresh terminal with full scrollback from 120 to 80 columns, then build its display |
| `frame/*-panes-*` | Build a frame with 1/2/4 panes sharing one Rust buffer or showing distinct buffers; total width grows with pane count |

Rust and TSX fixtures nest their items 100 to a `mod` or component, each line still one item with Unicode. Before the refactor-scan fixes every item sat at the tree's root, and a revert's changed lines keep their code and gain a trailing comment, so the nesting stays intact. Earlier Rust/TSX `syntax-*`, `revert-*` and `frame/*-rust` results are not comparable.

Highlight load timings use **warm grammars**: lazy query compilation is completed before measurement. They measure full file parsing, not first-ever language initialization. Edit timings include deferred parsing through `Core::frame`; a direct buffer edit alone would miss it. Finder results include matching, polling and frame construction, with repoll deadlines advanced rather than sleeping for the UI's 16 ms timer. File preview reads are not answered in the finder benchmark; it measures in-memory completion, not disk preview latency.

Observable self-checks cover text restoration, highlighted spans, visible text, completion counts, revert results, scrollback contents, terminal marks/cwd/size and pane counts. These checks fail the benchmark if it measures the wrong operation.

## Filesystem services

The service suite creates two roots with 5,000 source files each, plus `.gitignore` and excluded files. Both gitignore and project ignore patterns are exercised. Each source file contains 50 ordinary lines followed by one Unicode search hit.

| Benchmark | One measured operation |
| --- | --- |
| `index/10k-files-two-roots-warm-cache` | Walk and sort both roots; assert 5,001 indexed entries per root including `.gitignore` |
| `grep/10k-hits-two-roots-complete-warm-cache` | Search both roots to the production 10k hit cap, consuming result batches |
| `grep/first-batch-two-roots-warm-cache` | Search until the first nonempty emitted batch, then cancel; includes cancellation/unwinding overhead. The first hit is sent at once, so this is time to the first hit; results before the refactor-scan fixes include the 50 ms batch deadline and are not comparable |
| `*-4-workers` | The same two grep operations with `grep_workers = 4`; the names without a suffix use one worker |

Fixture creation and initial validation run before measurement. These are **warm filesystem-cache** workloads; they do not flush OS caches or claim cold-disk performance. First-result timing includes the service's batching policy, but excludes the prompt's 200 ms input debounce, channel scheduling and GUI painting. The complete search verifies hit counts, text and positions.

### Grep workers

Medians (min–max) in ms, release build, warm cache, recorded 2026-10-30 at the parallel grep change ([#265](https://github.com/Root-man/aiditor/issues/265)):

| Platform | Workers | Complete search | First batch |
| --- | ---: | ---: | ---: |
| Linux 7.2 (Fedora 44), AMD Ryzen 7 5800X3D, 16 threads, Rust 1.99.0 | 1 | 93.9 (91.2–101.7) | 0.40 (0.39–0.58) |
| Linux 7.2 (Fedora 44), AMD Ryzen 7 5800X3D, 16 threads, Rust 1.99.0 | 4 | 29.9 (28.6–33.9) | 0.45 (0.43–0.47) |
| macOS | 1 | not yet measured | not yet measured |
| macOS | 4 | not yet measured | not yet measured |

Workers overlap file opens and searches, so they shorten complete searches; the first hit already arrives at once, so the first batch gains nothing and pays for starting the workers.

### Sparse project grep investigation, 2026-10-31

The saved **Legacy crap** project has 11 roots and 8,705 searchable files. `INVALID_SITE` occurs on 48 lines in two later roots; six preceding roots contain 7,225 match-free files. The release service was measured on Apple M5 Pro, macOS 27.0.1, Rust 1.99.0, with four configured workers and warm filesystem caches. These headless callback times exclude the prompt's 200 ms debounce and GUI painting. The full [project grep aggregate](measurements/2026-10-31-project-grep.json) retains the investigation's baseline runs and comparisons.

| Release service, four workers | First callback, median ms | Complete, median ms |
| --- | ---: | ---: |
| Before: serial walk and ordered delivery, five runs | 399.2 | 458.8 |
| After: parallel walk and arrival-order delivery, five runs | 36.5 | 408.9 |

The new first callback is about 11 times earlier on this workload. A service-to-core run returned its first hit at 38.8 ms; core handling and frame construction then took 0.015 and 0.065 ms. Its first preview read and installation took 0.18 and 11.2 ms. These are core-side timings, not a GPUI paint measurement. The final service run returned the same 48 file/line locations as the saved baseline, though their arrival order changed. No GUI painting run was made.

The one-worker repeat took about 1.04 s to first callback and 1.25 s to completion, slower than the earlier 747/863 ms baseline. The diagnostic parallel walk on the same current host took 1.04/1.25 s too. This points to changed host file-open costs between runs rather than an identified new application bottleneck; it does not establish a stable one-worker speedup or regression. The four-worker complete search improved by about 11% in the saved before/after runs, while first-hit scheduling changed much more. Repeat measurements on a stable host before treating smaller completion differences as durable.

The [file-open aggregate](measurements/2026-10-31-file-open.json) records a roughly 81 µs mean for successful opens over 2,048 fixture files, with kernel stacks waiting for Endpoint Security authorization. The supplied [Defender trace aggregate](measurements/2026-10-31-defender-trace.json) contains 45,706 complete fixture `AUTH_OPEN` pairs and a 74.26 µs mean reported authorization latency. The separate captures do not split total open cost between Defender and the other active Endpoint Security client. No security setting was changed, and the application scheduling change does not remove host authorization latency. The supplied raw trace archive stays outside this repository.

For repeatable synthetic workloads, run the supported service benchmark:

```sh
python3 scripts/bench_local.py --suite services
cargo bench --locked -p aiditor-services --bench services
```

The suite now includes a sparse hit only in the second root, reporting first-callback and complete-search times separately for one and four workers. Its dense first-batch metric includes cancellation teardown. No CI timing threshold is imposed. The synthetic roots and cache behavior differ from the saved real project; neither headless benchmark measures GPUI painting.

| Release service benchmark, warm cache | One worker, median ms | Four workers, median ms |
| --- | ---: | ---: |
| Dense 10k-hit complete search | 1108.6 | 390.5 |
| Dense first batch, including cancellation teardown | 1.05 | 1.73 |
| Sparse later-root first callback | 688.3 | 188.9 |
| Sparse later-root complete search | 1075.7 | 356.4 |

These 2026-10-31 synthetic runs have no matching sparse baseline in the previous benchmark definition. The real project table above supplies the before/after sparse comparison. Dense completion is about 15% slower with one worker and 8% slower with four than the earlier macOS values. The same current-host diagnostic walk tracked the new production service on the real project, so host file-open variation remains a plausible cause; the synthetic and real workloads do not isolate it.

## Sampling

Reusable operations warm up by doubling the iteration count until a batch takes at least 10 ms, then measure 21 equal-sized batches. Revert application and terminal resize require fresh state: they use three warm-ups followed by 21 single-operation batches, with setup and teardown outside timing. First-batch grep also uses this single-operation sampling mode.

Results are median/minimum/maximum batch averages in ns/op. These ranges are not per-event latency percentiles or statistical confidence intervals. Self-checks inside an operation contribute to its timing. No timing threshold fails ordinary CI. Avoid other builds and heavy applications when collecting results for comparison.

## GUI input during terminal floods

```sh
python3 scripts/bench_local.py --suite ui --output /tmp/aiditor-ui-samples.tsv
```

This opens an isolated real GPUI window with a 10k-line highlighted Rust buffer. It uses default settings, no user configuration/recovery, and a temporary cat fixture. It runs three sequential scenarios: no flood, a real PTY running `yes`, and a real PTY repeatedly running `cat` on an approximately 32 MB file. The terminal buffer is hidden while the source buffer stays visible. Each scenario warms for two seconds and collects 120 synthetic input samples, scheduling the next input 50 ms after the previous sample is painted. Keep the window visible and do not interact with it. Processes are killed between scenarios; the app exits after writing the report. A 90-second watchdog fails a run that cannot paint.

The start timestamp is the intended input delivery time on the background task. The end is completion of the editor element's paint callback for the frame containing the matching probe text. This includes scheduler delay, core work, frame construction and text shaping/paint submission. It **excludes OS keyboard delivery, GPU completion and display scanout**. One input is outstanding at a time, so superseded edits are not counted as displayed inputs.

The separate report shows sample count and nearest-rank p50/p95/p99/max for input-to-paint latency and consecutive paint gaps. Gaps include the input cadence and idle periods; they are not GPU frame durations. Processed terminal byte counts verify that each flood produced output. Raw GUI TSV uses `scenario`, `metric`, `value` (ns for timings; bytes for output counts); it cannot be mixed with the headless TSV or passed to the batch comparison helper. Record OS, hardware, display refresh rate, window size and power mode alongside results. GUI measurements are local-only and do not run in hosted CI.

## Run in GitHub Actions

After `benchmarks.yml` is merged into the default branch, open **Actions → Benchmarks → Run workflow**, select the branch to measure, and optionally supply its PR number. This is a manually started workflow, without a separate environment approval gate. It does not run on pushes or PR events.

The CLI equivalent is:

```sh
gh workflow run benchmarks.yml --ref your-branch -f pr=123
```

Omit `-f pr=123` for a summary without a PR comment. The tested commit is the selected branch's commit when dispatched. Fork PRs are not supported: the optional PR must be open, in this repository, and still have that tested commit as its head when reporting. Invalid or stale targets fail reporting after retaining the summary; they receive no comment.

The workflow uses `ubuntu-24.04`, builds and runs the core and service benchmark targets, and caches its build artifacts separately from ordinary CI. The benchmark job has read-only repository permissions. A separate reporting job can read Actions artifacts and write PR comments. No hosted workflow is dispatched by local development commands.

Every successful benchmark job uploads `results.json` as `core-benchmarks`, retained for 90 days (subject to repository retention policy). The report always goes in the reporting job's summary; providing a PR number also creates a new PR comment. Re-running a workflow can create another comment.

The baseline is the previous successful `Benchmarks` workflow run on the **same branch**, excluding the current and later run IDs. It is not the PR's base branch. Failed runs are skipped. The first run, a missing/expired artifact, or changed benchmark definitions/environment establishes a new baseline with no percentage comparison. Other API or malformed-artifact errors fail reporting rather than silently claiming there is no baseline.

Comparisons require matching runner label, architecture, CPU model, full Rust compiler version and a fingerprint of both benchmark suites, shared helpers, local runner, crate manifests and workspace build profiles. Results also record the tested commit, run ID and runner image version; dependency or production-code changes can therefore be measured without changing the benchmark fingerprint. The runner image version is informational and does not prevent comparisons. Positive changes mean slower; negative changes mean faster.

Hosted runners vary even with matching metadata. Treat changes as clues, repeat suspect measurements, and confirm meaningful regressions on consistent hardware. The headless suites measure core operations and warm-cache filesystem services; they do not establish GUI latency. The explicit GUI suite below measures scheduled synthetic input through frame paint, not physical key-to-photon latency.

## Reporting checks

The report helper uses Python's standard library and GitHub CLI. Its recording command is Linux CI plumbing, reading CPU information from `/proc/cpuinfo`; local benchmarking does not need it. Ordinary CI runs its small unit suite:

```sh
python3 -m unittest discover -s scripts -p test_benchmarks.py
```
