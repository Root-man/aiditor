# Tasks

## 1. Shared coverage and concurrent traversal

- [x] 1.1 Adapt the existing file-walking helpers for multi-root grep with root-relative project patterns, preserving the single-root index/watcher callers; verify coverage against `files::index` on two roots with nested ignore files, allow-list rules, hidden files, `.git`, file symlinks and directory symlinks, and run the existing file/watcher tests.
- [x] 1.2 Replace grep's serial walk and ordered per-file queues with one configured parallel walk and a bounded shared hit channel, keeping a reusable searcher per visitor; verify uncapped complete-set equality across worker counts, per-file line order, Unicode/CRLF/smart-case/binary behavior and the configured active-file bound, and delete helpers left without callers.

## 2. Timed collection, cap and shutdown

- [x] 2.1 Implement immediate first-hit delivery and an approximately 50 ms collector timeout independent of traversal and file reads; verify a first hit arrives immediately, a pending second hit arrives while producers remain stalled, and a continuously active producer still flushes on the deadline.
- [x] 2.2 Make the collector enforce first-received cap membership and exactly one successful done message; verify limits 0, 1, 3 and 10,000 with competing producers, plus empty roots, no matches, unreadable entries and invalid regex completion.
- [x] 2.3 Set stop state and drop the result receiver before joining workers on completion, cancellation, output closure and unwinding; verify cancellation and cap termination with saturated queues, cancellation while awaiting events, and the existing newer-search/cancel supersession behavior using bounded test timeouts.
- [x] 2.4 Replace obsolete root/file-order and cancellation-check-count assertions with observable behavior checks; verify a hit from a later root reaches the callback while an earlier file is held at a controlled barrier, then release it and verify the final complete hit set.

## 3. Core behavior and lasting performance coverage

- [x] 3.1 Add a service-to-core streaming check and extend core tests with interleaved file/root batches and completion after selection moves; verify growing counts and `[searching]` during the run, stable selected hit/preview/rows, retained arrival order at completion, empty/error states and stale-reply rejection without changing the message API.
- [x] 3.2 Add a sparse later-root case to `crates/aiditor-services/benches/services.rs`, retaining dense cases and the existing output conventions; verify expected hits and record first-callback latency separately from completion/cancellation teardown for one and four workers, with no CI timing threshold.
- [x] 3.3 Build the release service and repeat `INVALID_SITE` against the saved Legacy crap roots while the probe still exists; retain before/after first-hit and completion measurements, compare the full file/line set with the recorded 48-hit baseline, and explain any changed input files. Report headless callback timing separately from any GUI painting check.

## 4. Diagnostic cleanup and documentation

- [x] 4.1 After transferring useful checks and retaining the final measurement, remove `crates/aiditor-services/examples/grep_probe.rs`, `crates/aiditor-services/examples/open_probe.rs` and `scripts/analyze_mde_trace.py`; verify the production tests and sparse benchmark cover the fix, no executable probe references remain in build targets, and the supplied trace archive and unrelated temporary files remain untouched.
- [x] 4.2 Update `docs/editor.md` for arrival-order display and first-received capped membership; condense the investigation additions in `docs/performance.md` to supported benchmark commands, aggregate results and limitations, retain the three existing JSON evidence files, and verify all retained links/commands resolve without the removed utilities.

## 5. Integrated verification

- [x] 5.1 Run `cargo fmt --all --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test` and `git diff --check`; verify the shared file/watcher and core/service suites pass and report the platform actually tested, leaving both macOS and Linux requirements represented in CI.
- [x] 5.2 Run the release service benchmarks and compare sparse first-hit and complete-search results with the baseline; verify the intended first-hit improvement and investigate material full-search regressions while distinguishing remaining host file-open costs from application scheduling.
- [x] 5.3 Check the final implementation against every delta scenario and cleanup item, then run `openspec validate stream-project-grep-as-found --strict`; record completed checks and any explicit runtime limitations, without claiming that the application change removes Defender latency.

## Verification notes

- Tested on macOS 27.0.1 (Darwin), Rust 1.99.0. `cargo fmt --all --check`, `cargo clippy --all-targets -- -D warnings`, `git diff --check` and strict OpenSpec validation passed. CI still targets both macOS and Linux; Linux was not run locally.
- `cargo test -p aiditor-core -p aiditor-services -- --test-threads=1` passed, including all file, watcher, grep, core and service integration tests. The final two output-closure and cap tests also passed in the 22-test grep suite.
- Unfiltered `cargo test` was run but did not finish: `editor::tests::every_bundled_binding_reaches_the_core` stalled during runtime shutdown, and the same isolated test timed out at untouched commit `aeca08f`. A serial run skipping it passed 36 GUI tests; `integrated_finders_visits_saves_and_directory_recovery` hit its native watcher readiness timeout once, then passed in isolation. No full-suite pass is claimed.
- The real-project release service returned the same 48 file/line locations as the saved baseline. Headless callback times exclude GUI paint; no GUI painting check was run. The supplied trace archive and unrelated temporary files were not modified.
