Scope: The whole project at `d3c9565` (`Performance benchmarks (#264)`), refreshing the scan made before #108 and M2; recorded on 2026-10-25, with the reported macOS/Linux grep comparison investigated from source without rerunning benchmarks.

What this code does: aiditor is a keyboard-first Rust editor, with editor state in `aiditor-core`, filesystem/process work in `aiditor-services`, and a GPUI frontend in `aiditor`.

| # | Opportunity | Category | Where | Impact | Complexity | Fix |
|---|---|---|---|---|---|---|
| 1 | Normal shell exit leaves PTY handles and input threads alive until application shutdown. | Bug | [core/terminal.rs:716](../crates/aiditor-core/src/terminal.rs#L716), [services/terminal.rs:134](../crates/aiditor-services/src/terminal.rs#L134) | high | med | Send the existing terminal cleanup effect on exit and failed spawn before forgetting the terminal; verify resource release. |
| 2 | Recovery panics when the visited disk file has become binary/read-only. | Bug | [core/autosave.rs:60](../crates/aiditor-core/src/autosave.rs#L60) | high | med | Use the existing undoable `Buffer::replace` operation, which supports replacing read-only content, and cover recovery into a refused disk file. |
| 3 | Long-line redraw scans the hidden prefix and builds highlights outside the horizontal viewport. | Performance | [core/display.rs:154](../crates/aiditor-core/src/display.rs#L154), [core/display.rs:234](../crates/aiditor-core/src/display.rs#L234), [core/syntax.rs:209](../crates/aiditor-core/src/syntax.rs#L209) | high | med | Skip hidden prefixes directly where folds permit; limit highlight painting and conversion to the displayed ranges while retaining syntax context. |
| 4 | Terminal copy mode repeatedly counts an increasingly long soft-wrapped logical line. | Performance | [core/terminal.rs:296](../crates/aiditor-core/src/terminal.rs#L296) | high | med | Track the logical line's character count as rows are appended, resetting at a hard newline; preserve Unicode positions and colours. |
| 5 | Project grep serializes traversal and file searches; the reported 10k-file completion takes about 1 s on macOS. | Performance | [services/grep.rs:88](../crates/aiditor-services/src/grep.rs#L88), [services/files.rs:212](../crates/aiditor-services/src/files.rs#L212), [services bench:59](../crates/aiditor-services/benches/services.rs#L59) | high | med | Proposed: retain the ordered walker and overlap file searches with a small bounded worker set, reusing one searcher per worker and streaming results through an ordered coordinator. Validate the benefit before adopting; details below. |
| 6 | A failed replacement recovery write can be followed by deletion of the old recovery file. | Risk | [core/autosave.rs:25](../crates/aiditor-core/src/autosave.rs#L25), [core/autosave.rs:73](../crates/aiditor-core/src/autosave.rs#L73), [services/recovery.rs:21](../crates/aiditor-services/src/recovery.rs#L21) | high | high | Acknowledge successful persistence before marking a buffer tick auto-saved or deleting its previous recovery copy; retain the old copy and allow retry after failure. |
| 7 | `char_idx` walks the whole line just to inspect its final character. | Performance | [core/buffer.rs:639](../crates/aiditor-core/src/buffer.rs#L639) | low | low | Read the final character by index, handling an empty slice, instead of calling `chars().last()`. |

One smaller opportunity omitted: display construction collects all copy-mode properties before filtering to the viewport. File size alone was not treated as a finding.

#1: The core removes the terminal mapping before buffer cleanup, suppressing the cleanup effect. The service removes its retained handle only on `Kill` or `kill_all`; the input thread waits on a sender held by that handle. An earlier probe on this revision ran twelve `/bin/sh -c 'exit 0'` terminals: open descriptors rose from 9 to 33 and returned to 9 after `kill_all`.

#2: An earlier probe offered valid recovered text for a disk file containing bytes `[0, 1, 2]`. Opening it produced a read-only buffer; recovery's `insert(...).expect(...)` panicked with `ReadOnly`, leaving the buffer empty. `Buffer::replace` already has a test for an undoable replacement past read-only.

#3: Earlier optimized core-display probes, with parsing outside the timing, took about 41–44 ms per frame near the end of a 1 MB single line. The source walks each hidden prefix character; syntax also paints an array covering the requested whole lines. These are core display timings, not GPUI frame measurements.

#4: Earlier optimized copy-text probes with 120-column rows took about 5.5 ms for a logical line spanning 1,000 rows and 216–221 ms for 10,000 rows. Recounting the growing prefix on every wrapped row makes that part quadratic.

#6: An earlier probe forced the replacement write to fail by occupying its temporary-file path with a directory. The recovery worker logged the failure and processed the following deletion, leaving no old or replacement recovery copy. The recovered buffer still exists in memory, but a subsequent crash can lose its only remaining copy.

## Grep investigation (#5)

The supplied macOS result for `grep/10k-hits-two-roots-complete-warm-cache` is **999.674 ms median, 932.436 ms minimum, 1191.942 ms maximum**. Linux was reported at approximately **95 ms**, giving roughly a **10.5×** difference using the supplied median. No benchmark, timing probe, or profiler was run for this investigation. The platform cause and the proposed speedup remain unmeasured.

### What the source establishes

- [The fixture](../crates/aiditor-services/benches/services.rs#L11) contains two roots, each with 5,000 source files in 50 directories, plus ignore files and excluded files. Each source file is 1,063 bytes with one hit after 50 ordinary lines. This is about 10 MB distributed across 10,000 small files.
- [The timed operation](../crates/aiditor-services/benches/services.rs#L59) calls `grep::search` directly. It includes regex construction, ignore-aware traversal, sorting, opening and reading files, matching, hit allocation, clock checks, and consuming/validating batches. It excludes Tokio task/channel scheduling, the prompt debounce, and GUI rendering. Python launches Cargo's benchmark target and formats its results; Python is outside the Rust operation timer.
- [The search loop](../crates/aiditor-services/src/grep.rs#L88) walks and searches one file at a time, across roots in order. The matcher and searcher are already reused across files within each query. In locked `grep-searcher` 0.1.17, `Searcher::search_path` opens a file and uses buffered reader search with this configuration; memory mapping defaults to disabled on both platforms. Enabling mmap is not a macOS fix: that version also explicitly declines mmap on macOS.
- [The benchmark helper](../benches/support.rs#L3) performs warm-up and then 21 measured batches on the same fixture, after an initial complete validation search. Creation is outside timing. Dividing 999.674 ms by 10,000 gives about **100 µs per source file for the whole workload**, not a measured open/read/close duration. The repeated measurements do not rule out ongoing filesystem/security overhead, but they weaken a simple first-read-only explanation. Running the wrapper again would create a new fixture and would not isolate security scanning.
- [The specification](editor.md#subsystems) and [tests](../crates/aiditor-services/src/grep.rs#L203) require root order, the existing walk order, and line order. Locked `ignore` 0.4.33 documents that `sort_by_file_name` is ignored by `build_parallel`. Simply changing that builder would violate current behavior. The existing serial-walk shortcut is worth revisiting because a slow target-platform workload has now been reported.

The dependency checks above used the installed source matching [Cargo.lock](../Cargo.lock): `grep-searcher`'s `src/searcher/mod.rs` and `src/searcher/mmap.rs`, and `ignore`'s `src/walk.rs`.

### Candidate explanations and distinguishing evidence

These are investigation priorities, not established causes; none was tested by a new run:

1. **Serial per-file I/O overhead.** If opening/reading/closing dominates, a profile will spend most elapsed time there, and searching an already enumerated path list will retain most of the cost. This is the strongest code-level candidate given many small files and serialized searches, but it does not identify why the two machines differ.
2. **Traversal and ignore-processing overhead.** If this dominates, walking the same fixture without opening file contents will retain much of the delay. The existing index result is a useful clue, though it is not an exact subtraction because indexing also collects directories and sorts its final lists.
3. **Matching, allocation, or machine/build differences.** If the gap persists when searching preloaded bytes, investigate CPU work and verify compiler, architecture, build settings, and system load before attributing it to filesystem calls. The supplied output alone does not control these differences.
4. **Temporary-directory or security-software effects.** If these dominate, a filesystem trace/profile and a controlled change of fixture location should expose them. There is no captured evidence identifying Endpoint Security or XProtect as the cause. A faster second invocation alone would not establish that attribution.

### Proposed change and acceptance criteria

Keep the current ignore-aware ordered walk initially. Feed a bounded number of upcoming files to a small worker set; each worker reuses its searcher. A coordinator emits results in the existing order and owns the public `cancelled` and `send` callbacks, keeping `search` usable by its current production, headless, benchmark, and test callers. Bound both in-flight files and result queues. Workers must send incremental chunks: collecting an entire file's hits before delivery would regress the existing streaming behavior. This overlaps file work without requiring a new dependency, content cache, or change to the core protocol.

This proposal targets serialized file work; it cannot remove a traversal bottleneck. Ordered delivery can also wait for an earlier slow file. Choose the worker count and retain the change only after approved measurements show a completion-time improvement without harming first-result latency or resource use. A 10× speedup is not established.

- **Given** files in several roots whose searches finish out of order, **when** results are delivered, **then** their root, walk, and line order matches today's service.
- **Given** one large matching file, **when** it is still being searched, **then** hits stream at the existing approximate 50 ms checks rather than waiting for file completion.
- **Given** a superseding query or cancellation, **when** workers and the coordinator observe it at their file/match/send checks, **then** they stop work and the coordinator emits no further results for that search.
- **Given** more than 10,000 hits, **when** the default search reaches its cap, **then** exactly the first 10,000 ordered hits and one final completion are delivered.
- [ ] Keep ignore rules, binary handling, Unicode columns, unreadable-file handling, and the existing callback API behavior covered by the service tests.
- [ ] Bound worker count, queued work, and buffered results, including cancellation while a queue is full.
- [ ] After explicit approval to measure, compare complete-search and first-batch timings on each platform with a single-worker baseline and a few bounded worker counts; keep fixture and build conditions constant within each comparison.

## Verification and limits

The earlier scan on this revision passed 570 Rust core/service tests with one ignored, plus 21 Python benchmark-reporting tests. Focused probes established the cases described above; they were not rerun for the grep investigation. The new investigation checked the benchmark boundaries, locked dependency implementations, callers, ordering tests, and cancellation/streaming tests from source. Only this report was added; no proposed fix was implemented.

Start with: **1 and 2**, because existing cleanup/replacement operations address concrete bugs with limited changes. **6** also deserves early attention because failed recovery persistence can remove the remaining on-disk copy; **5** is the proposed response to the reported macOS grep delay, subject to measurement.

Not checked: GUI runtime behavior, Linux execution, or a new macOS benchmark/profile; the optional `ponytail-audit` skill was unavailable. The actual cause of the macOS/Linux gap and the benefit of parallel search are unverified.
