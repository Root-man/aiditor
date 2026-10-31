# Design

## Context

See `proposal.md` for motivation and the approved ordering decision. The production service in `crates/aiditor-services/src/grep.rs` walks roots serially, queues at most `4 * workers` files and drains only the first file's bounded hit channel. Its batch deadline is checked when an event is processed. The core already appends `Msg::Grepped` batches and preserves selection; no replacement message protocol is needed.

`files::walker` supplies hidden-file, ignore and `.git` filtering for indexing and watcher paths. Its project-pattern filter captures a single root. The diagnostic parallel prototype does not implement those project patterns, cancellation or a cap and must not be copied as a production replacement.

The locked `ignore` 0.4.33 provides a multi-root parallel walker and per-thread visitors. Its parallel builder ignores sorting and maintains work-stealing path queues. Those queues have no fixed capacity. Its current code must inform resource claims rather than treating a parallel walker as a bounded job channel.

## Goals / Non-Goals

**Goals:** Make ready matches available independently of other files; keep one collector responsible for message order, time deadlines and the cap; share existing coverage rules; retain a small synchronous service API callable from the blocking pool and tests.

**Non-Goals:** A new search engine, scheduler framework or configuration surface; a deterministic final subset; re-sorting or file grouping; preempting an OS-blocked open/read; changing index/watcher ordering or the bundled worker count. Defender authorization overhead remains a host constraint.

## Decisions

### 1. Search inside a parallel multi-root walk

Build one `ignore::WalkParallel` over all roots with `threads(op.workers.get())`. Each visitor owns a reusable `Searcher` and searches eligible files directly. Keep the shared compiled regex and current smart-case, binary, CRLF and character-column behavior. There is no second search pool or application file-job queue.

Factor the minimum common filter/file-kind logic in `files.rs` so grep and the index use the same rules. For grep, resolve each entry against its containing configured root, and apply `Patterns` to that root-relative path. Preserve hidden entries, `.git` exclusion, ignore-file precedence, file symlinks and skipping directory symlinks. Configured roots are already validated as nonoverlapping lexical paths; do not canonicalize roots or follow directory symlinks as part of this change. Preserve the existing single-root `walker(dir, root, patterns)` behavior used by inventory and both watcher call sites. Remove `files::walk` only if its last caller disappears.

Alternative: keep serial traversal and only merge ready file channels. This removes file-level withholding but leaves the measured late-root discovery delay. Separate pools per root would multiply the worker budget. A complete prebuilt inventory delays the first hit. The existing parallel library offers the tested benefit with less new scheduling code.

### 2. One bounded hit channel and one timed collector

Run the walker on a scoped producer thread while the calling thread collects from a bounded synchronous hit channel. Visitors send hits immediately in per-file line order. A fixed hit-channel capacity, such as the existing 64-hit buffer size, provides backpressure without per-file queues. The collector invokes the existing `FnMut(Msg)` callback, so callers need no new thread-safety bound.

On the first received hit, send immediately. Afterwards, use `recv_timeout` against the next deadline and check that deadline after successful receives as well, so a continuously busy channel cannot starve flushing. Timeouts flush only nonempty batches; when idle they also check cancellation at intervals no longer than the batch interval. A late hit after an expired deadline flushes immediately. Completion drains the accepted pending hits in exactly one final done message. Empty roots and zero limits complete directly; invalid patterns keep the current single-error completion behavior.

A timer on the same thread as walking or reading would still be blocked by filesystem calls. The collector can continue while those operations wait; downstream callback backpressure and OS scheduling remain explicit limits.

### 3. The collector owns cap and shutdown decisions

Only the collector counts accepted hits. Stop at `op.limit` even when several workers have results queued, deliver the final accepted batch once, and discard later arrivals. Set a shared stop flag and drop the hit receiver before joining the producer so blocked worker sends fail and unwind. Apply the same shutdown ordering on cancellation and output closure; cancellation emits no completion. Workers check the stop/cancellation condition at traversal callbacks, before a file and within matching-line callbacks, returning `WalkState::Quit` or stopping the sink as appropriate.

Keep `Grep::run`'s search identity and core-side stale-reply rejection. No shutdown mechanism can interrupt an already blocked kernel read; it must not depend on another matching line or queued send to notice cancellation once work resumes. Use scoped threads and ensure receiver drop also occurs during unwinding.

### 4. Append-only UI behavior

Keep `Msg::Grepped`, `Hit`, `GrepSearch` and `Core::on_grepped`'s append path. A first reply replaces old results; later and final replies append. Do not sort on completion, which would move rows and require additional selection reconciliation. Test interleaved files/roots and a final batch after selection moves. Keep the 200 ms debounce, 10,000-hit UI limit, preview navigation and `[searching]`/`[no matches]` behavior.

### 5. Replace probes with durable, focused checks

Replace ordering-sensitive service tests with complete-set equivalence for uncapped searches, per-file line-order checks and exact arrival-order cap tests. Use controlled channels/barriers or a minimal private file-boundary test seam to prove a later-root hit is delivered before an earlier file is released, and a pending hit is flushed while producers remain stalled. Avoid assertions about cancellation-check counts or thread identities. Keep bounded timeout guards so a broken implementation fails rather than hanging tests.

Exercise the production service callback directly for streaming tests. `aiditor-headless` and `tests/support::Driver` collect messages until synchronous search returns, so they cannot prove incremental delivery; rewriting those general runners is unnecessary. Feed batches immediately into a core in the integration check.

Extend the existing service benchmark with a sparse pattern appearing only in a later root, retaining dense first-batch and full-search cases. Report time to the first callback separately from total completion and cancellation/join time. Reuse existing sampling/output conventions; add only the small timing helper needed if those conventions cannot express callback timing. Benchmark one and four workers without timing assertions in CI. Repeat the saved `INVALID_SITE` workload before removing the probe and compare the complete 48 file/line locations, subject to source-tree changes being recorded.

After validation, delete the two examples and the Defender decoder. Preserve the aggregate JSON evidence, condense `docs/performance.md` to methodology/results/limitations plus supported benchmark commands, and remove dead links and reproduction commands for deleted utilities. Leave the supplied archive and unrelated `/tmp` files untouched.

## Risks / Trade-offs

- Nondeterministic order and capped membership → User approved first-received results; document it in `docs/editor.md` and replace the old ordering contracts explicitly.
- Traversal path storage grows with directory breadth → Bound active searches, result channel and accepted hits; do not claim constant total traversal memory. The delta narrows the previous file-queue guarantee to the actual architecture. A hard total traversal-memory bound would require a different scheduler and is outside this change.
- Root-relative ignore mistakes could expose excluded files in results → Reuse the existing policy and compare searched paths against the index across multiple roots, including allow-list and nested ignore cases.
- Blocked sends or incorrect sender lifetimes could hang shutdown/completion → Drop the collector receiver before joins, drop the producer's original sender after traversal, and test cap/cancellation with saturated queues.
- Real-time tests can be flaky → Prefer controlled scheduling and event ordering; use generous failure timeouts, not tight performance thresholds.
- One worker cannot search around its own blocked read, and multiple workers can all be blocked → Preserve the configured budget; guarantee delivery of already received hits, not arbitrary filesystem progress.
- Prototype timings are not a production performance guarantee → Measure the final release build and separate headless delivery from GUI paint. No policy changes or A/B/A tests are required.

## Migration Plan

No settings or stored-data migration. Implement and verify on `investigate/project-grep-latency`, update docs with the changed ordering, and remove temporary diagnostic code only after its useful assertions and final measurement are retained. Rollback is a code revert of the service change and its matching specification/documentation update; aggregate investigation evidence remains useful independently.

Structural acceptance checklist:

- [ ] Existing service/message API and configured worker count remain intact.
- [ ] Coverage policy is shared; index and watcher contracts remain unchanged.
- [ ] No per-file ordered result queues, complete pre-search inventory or final re-sort.
- [ ] Result buffering, the exact cap and cooperative teardown are checked.
- [ ] Temporary probes/decoder are removed after durable tests and measurements exist.
