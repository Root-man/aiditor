# Design

## Context

See [proposal.md](proposal.md) for motivation and scope. The source baseline is `6c0ef19`. The six findings cross core display construction, syntax snapshots, asynchronous recovery and benchmark tooling; a design is needed to preserve the existing contracts at their boundaries.

The core owns buffers, syntax and frames and has no GPUI or Tokio dependency. Services execute effects; the frontend feeds their messages back to the core. Recovery currently uses a sequential standard-library channel and worker, which is joined on normal quit. Keep these boundaries.

### Evidence and limits

The supplied [benchmark report](../../../docs/measurements/2026-11-01-main-benchmarks.md) records a 892.180 ms median for the 100,000-line Rust character-edit/undo frame pair and 1,053.176 ms for TSX. Its comparison with PR #314 is cross-revision and lacks matched historical host metadata; it does not establish attribution. Its companion patch changes only obsolete benchmark assertions.

The scan also ran separate release probes against the current sources. The timing rows below are medians of five samples on this machine, measuring core work, not GPUI shaping or painting. They are diagnostic observations, not the report's 21-batch benchmark results or performance acceptance thresholds.

| Finding | Source mechanism | Focused observation |
| --- | --- | --- |
| 1. Local highlighting | `syntax.rs::edit` clears all locals; `locals` walks the root and starts a highlight query for each definition | A space edit among 100,000 Rust function lines took 0.262 ms to reparse, 455.318 ms for its first 50-line highlight and 0.001 ms for a cached highlight. At 10,000 lines: 0.034 / 42.554 / 0.001 ms. |
| 2. Preview overlays | `display.rs` collects every match on a full line and converts every byte range to character positions | A stationary Swiper preview of `x` on a line of 100,000 `x` characters took 14.466 ms per frame; 1,000,000 took 132.273 ms. |
| 3. Hidden streams | `frame` calls `render_streams` for every session; a dirty message reparses its entire Markdown block | Adding five characters to a roughly 5,000-line styled reply cost 50.653 ms in a shown session and 49.817 ms in a hidden one; clean frames cost 0.145 / 0.005 ms. |
| 4. Autosave | `write_autosaves` records the tick before IO; the worker logs failures without replying | Obstructing the recovery temp path, then removing it, left a modified buffer with no recovery file; neither an explicit auto-save nor a later timer emitted a replacement write. Existing old-copy retention already works on the initial failed replacement. |
| 5. Swiper rows | `candidate` copies a full line, `highlights` paints its full range, and the frontend gives that string to `StyledText` | An empty-query first frame for ten Rust comment lines of roughly 1 MB each took 40.240 ms. Clipping in the frontend occurs after receiving the full strings. |
| 6. Benchmark contracts | Core benchmark assertions predate fixed-width counts and the finder's initially unselected state | Current Swiper returns `" 1/80000 "` where the checked-in benchmark expects `"1/80000 "`; an empty finder prompt shows `*/N`. |

The syntax probe uses 100-line modules containing `fn item_N() -> usize { let café = N; café + 1 }`, edits near the middle and highlights 50 lines. The session probe repeats headings, styled prose, links, Rust fences and lists, then appends ` next`. The result-row probe uses ten `//` lines followed by one million `x` characters. These fixture descriptions are retained here so implementation does not depend on a temporary diagnostic checkout.

Existing checks during the scan passed: core library tests, 716 passed and 1 ignored; recovery service unit tests, 3 passed. Those checks establish the baseline only. The planned fixes and their new tests have not run.

The installed GPUI 0.2.2 source confirms that overflow clipping alone does not truncate the input to text shaping. Existing editor coordinates count characters as columns; this change retains that model rather than introducing grapheme-width layout.

## Goals / Non-Goals

**Goals:**

- Make repeated display work proportional to the affected scopes or displayed text wherever the current semantics allow it.
- Keep complete source context for regex matching and syntax resolution, including offscreen definitions and injected languages.
- Represent recovery persistence separately from requested IO, with deterministic completion, retry and cleanup behavior.
- Make the existing benchmarks runnable and add focused measurements with explicit provenance.

**Non-Goals:**

- Moving syntax, search or Markdown parsing to a new service; incremental Markdown parsing; changing tool-call terminal rendering; redesigning generic completion or the transcript model.
- Guaranteeing constant-time global regex matching, huge top-level scope edits, initial parsing or the first display of a long hidden reply.
- New commands, settings, dependencies, recovery file formats, or CI timing gates. The scan's omitted grep-list and terminal-theme candidates remain outside this change.

## Decisions

### 1. Reuse local-binding analysis by lexical scope and batch definition captures

Replace the single invalidated locals map with scope records containing scope structure, definition/reference captures and resolution results. Keep capture identities independent of the theme; apply the current palette at paint time. Query definition highlight captures together for each rebuilt region, preserving the existing pattern-priority and `local` predicate rules, instead of restarting a root query per definition.

Retain explicit edit ranges through reparsing. Reuse unchanged Tree-sitter subtrees and their scope records; store cached positions relative to the owning scope so an insertion before an unchanged subtree does not require rewriting all of its references. Recompute the changed scope's bindings and invalidate dependent descendants when its exported binding environment changes. Parent chains supply definitions outside the displayed range, including declaration-value ordering, shadowing and non-inheriting scopes. Resolve/query affected and newly requested scopes lazily; do not discover every scope from the root on each keystroke.

Tree-sitter structural changed ranges are not sufficient on their own: a same-shape identifier edit can change binding resolution. Explicit edits, changed scope boundaries and inherited binding changes all participate in invalidation. Rebuild conservatively for removed/reparented scopes, changed injection ranges/languages or a replaced tree whose reuse cannot be established. Routine edits inside one of many independent functions must retain the other functions' analysis. An edit to a shared outer binding can legitimately invalidate many dependent scopes.

Apply the same rules to `Injection` analysis. Cloned `Syntax` used by Swiper must remain an isolated snapshot: sharing immutable records is acceptable, but mutable invalidation and resolution state must not leak between the live buffer and the snapshot. Avoid caching final theme faces in the binding records.

Alternatives: simply batching definition queries reduces repeated traversal but leaves whole-file analysis on every edit; using only visible definitions breaks offscreen references; preserving the map solely on structural changed ranges misses renames. Scope reuse plus batched queries addresses both measured costs while retaining full semantics.

Validation compares highlights after edits with a freshly parsed copy of the same text, covering Rust, JavaScript, TypeScript/TSX, nested shadowing, same-length renames, definition-value ordering, edits before a scope, undo/revert and injected content. Diagnostic measurements should show reused independent scopes, not merely a faster full rebuild.

### 2. Restrict preview match materialization to displayed spans

Change the shared `display::Search` contract and both Swiper/grep implementations to accept the displayed byte interval(s) of a source line and return only intersecting, clipped nonempty matches. Carry whether a retained span belongs to the first positive match on the complete qualifying line; the first retained span is not necessarily the first match.

Keep matching against the complete line. Run exclusions and ordered-word semantics before selecting display spans; preserve anchors, smart case, invalid-regexp literal fallback and empty-match handling. Iterate positive matches in order, retain intersections only, and stop once later matches cannot intersect a displayed interval. Convert only the retained boundaries to character positions. For a partially visible first match, its visible intersection retains `match-current`; an offscreen first match never promotes a later visible match to that face. Share the result across the displayed portions of the same line in one frame.

Use the actual spans produced by horizontal scrolling, wrapping and folds. Do not truncate the regex input, alter point placement, or add persistent buffer properties. Full-line materialization and regex work may remain necessary; this decision bounds match storage and coordinate conversion, not all search computation.

Alternatives: matching only the visible substring changes anchors and exclusions; collecting all matches and clipping afterward preserves the measured allocation/conversion cost. A persistent match cache is unnecessary for this first change and adds invalidation obligations.

Validation uses dense long lines, horizontally scrolled first/later matches, partially visible matches, wrapped rows, Unicode boundaries and all existing Swiper/project-grep matching and cleanup cases.

### 3. Render streamed messages when a frame displays their buffer

Compute the active workspace's laid-out pane set before flushing message streams in `frame`. Resolve every pane's buffer, including inactive panes and the prompt's origin, and render each shown session at most once even if multiple panes show it. Do not use only the selected buffer or every window remembered in every workspace.

Skip only the streamed-message call in `render_streams` for hidden buffers. Leave their dirty flag set and keep inserting raw text immediately. The first frame that displays them consumes the accumulated dirty state before constructing their display. Leave the existing dirty tool-call flush behavior in place.

Keep `end_stream` unconditional: before a type switch, tool call, plan, permission UI or submitted prompt releases the stream marker, render the completed block even if hidden. Do not infer finalization from an end-turn notification when the current implementation retains the stream until another boundary. Killing a buffer removes its pending session state safely.

Alternatives: changing `render_stream` itself to skip hidden buffers would also break finalization; rendering only the active pane leaves visible inactive panes stale. Incremental Markdown parsing is substantially larger and is not needed to remove this unrelated-frame cost.

Validation covers hidden buffers and inactive workspaces, inactive splits, prompts, duplicate panes, switching back without another chunk, finalization and killing a hidden session. Retain the tests that earlier blocks keep their properties.

### 4. Acknowledge recovery writes and schedule retries in the core

#### State and messages

Keep per-buffer confirmed snapshot state separate from one in-flight request. Each auto-save carries a unique request id and captured buffer tick; completion echoes both plus persistence success/failure and any separate post-persistence cleanup error. A late result applies only to the still-current request. The identifier must distinguish a new attempt from an earlier attempt at the same tick.

An eligible buffer stays eligible until successful persistence. While its write is pending, suppress duplicate writes. If newer edits exist on success, arm the normal idle/maximum scheduling without needing new input. On failure, clear the pending request, retain the last confirmed tick and recovery-copy associations, report the failure, and schedule a retry deadline about two seconds later. Input cannot indefinitely postpone that failure deadline. Retry takes a fresh snapshot rather than replaying old text. Use the core's existing clock/timer machinery; no sleeps, runtime dependency or new settings in the core.

The core reports file and error through the existing echo/`*Messages*` path. Suppress identical repeated errors for the same buffer until success; a changed reason is reportable. Do not include file contents in diagnostics. An unavailable recovery service or failed operation enqueue produces a correlated failure instead of leaving an attempt permanently pending.

#### Worker and reply transport

Keep the existing FIFO recovery worker. Report persistence success only after writing, syncing and renaming the temp file. A failure deleting an earlier copy after rename is a cleanup error alongside successful persistence, not a failed snapshot. Retain the existing directory permissions, file header and PID collision protection.

Add a completion callback to `recovery::spawn`. In the GUI, capture the existing Tokio runtime handle and service sender; the callback schedules an async send and returns immediately. Do not call `blocking_send`, wait on a future or use lossy `try_send` for acknowledgements: `Effect::Quit` joins the worker while the UI receiver may no longer drain. Request ids make delivery order harmless, and the one-pending-attempt rule limits ordinary outstanding completion work per live buffer. The worker must finish even when completion delivery has not finished or the UI receiver is closed.

In `aiditor-headless`, deliver callbacks through a standard-library completion channel. Replace the timer-only sleep/park loop with receiving completions until the next timer deadline, then send `Msg::Tick`; when there is no deadline, wait for a completion. Keep shutdown joining the worker after queued deletes. Update the frontend constructor/effect dispatch, all worker test callers, core message handling and the harness together. The harness normally acknowledges successful writes, with explicit delayed/failing replies for race tests.

#### Recovery replacement and cleanup

Register an accepted earlier recovery file with its buffer before queuing the replacement. Keep that association through failures and while another attempt is pending; do not rely on mutating only the first emitted effect's `replaces` field. Preserve every still-needed old path if more than one recovery is accepted for the same buffer before completion. Only a successful snapshot that includes the recovered text, or an explicit save/discard, permits releasing its old copy.

When a buffer becomes clean, is killed or the app quits, retire its requests, cancel retry eligibility and enqueue deletion after any writes already sent to the same worker. Cleanup considers pending, failed and confirmed state, including old replacement paths. Late replies cannot reinsert state or schedule work. If the buffer was edited while an ordinary file save ran, it remains modified and retains recovery protection. A newer attempt for a still-live buffer uses a fresh id so a prior retired reply cannot acknowledge it.

Normal quit must not need replies to finish cleanup. Keep the operation queue ordering and join; test with more queued completion notifications than the GUI channel's capacity. Preserve the current behavior for actual filesystem cleanup failures: report them and retain the surviving copy, without pretending deletion succeeded.

Alternatives: treating enqueue as success retains the bug; blindly retrying old operations loses newer edits; blocking acknowledgements can deadlock quit; replacing the worker/thread architecture is unnecessary.

### 5. Build Swiper presentation from bounded source slices

Compute a result-row column budget from the reported frame width, existing completion insets and cell width. This is the width of the result row, not the prompt's remaining width after its label and count. Clamp narrow/zero-width layouts safely and recalculate after resize or font-size changes.

For Swiper rows only, select the prefix and source slice within that budget before allocating the displayed candidate or querying its syntax. Convert the source slice's character boundary to a valid UTF-8 byte boundary once; paint exactly those source ranges and translate/clamp their runs to the bounded presentation. Use the same clipped slice for text and faces. The frontend receives the short string and matching runs, while retaining its existing selection background and ordinary clipping.

Keep the complete search snapshot for filtering and syntax context. Keep selected-line identity separate from the clipped display string: selection, preview and acceptance must use the source index/canonical candidate path, including when the entire visible row is only part of the line-number prefix. Do not parse a truncated prefix to choose a target. Preserve live-buffer preview semantics and snapshot-based result styling when the origin changes.

Alternatives: frontend overflow still shapes full strings; slicing after `candidate` or `highlights` retains full-line core work; truncating the search snapshot breaks matches beyond the edge. A general completion-model redesign or new ellipsis behavior is unnecessary.

Validation covers ten very long rows, narrow and zero-width prefixes, full-line matches past the clipped edge, Unicode, resize/zoom, source mutation, snapshot locals/injections, current-theme changes and frontend syntax/selection runs. The syntax reuse work precedes performance acceptance here so scope discovery does not disguise row-clipping improvements.

### 6. Repair benchmark assertions before collecting comparisons

Apply the supplied report's expectation corrections in `crates/aiditor-core/benches/core.rs` and `benches/scenarios/mod.rs`: preserve the Swiper count's width eight (including the final `" 1/80000 "` check), and wait for `*/N` at an empty finder prompt versus `1/N` after typing. Ignore padding only in the finder's settle predicate, retaining its final total-count check and deadline condition.

Do not change existing fixture contents, names, release settings or sampling helpers to improve numbers. Add distinct focused cases using the existing benchmark support for scoped edit/highlight, dense preview matches, hidden streaming frames and long Swiper result rows. Autosave correctness belongs in deterministic core/service tests and a real worker retry test, not a timing threshold.

Measure a repaired-baseline checkout and the implementation on the same host/toolchain with the same fixtures, sequentially. Preserve raw samples and metadata. Separate first/dirty work from cached/clean work and core frame construction from GUI shaping/painting. Use the existing full suite to catch broader changes; report the existing GUI baseline/flood scenarios separately, and directly check long-row GUI presentation. No change to hosted workflow permissions or automatic comments is needed.

## Risks / Trade-offs

- **Stale local binding results** → Use explicit edit invalidation as well as tree reuse, verify against fresh parsing, and rebuild conservatively when scope/injection identity is uncertain.
- **Large shared scopes and first-load costs remain** → Document them separately from the independent-scope edit improvement; do not advertise constant-time highlighting.
- **Regex context still requires full-line work** → Bound retained spans and conversions while measuring the remaining scan cost honestly.
- **First display of a long hidden reply still parses accumulated Markdown** → Preserve finalization and correctness; this change removes repeated work from unrelated frames, not all transcript latency.
- **Persistence replies race with edits, saves and quit** → Correlate request ids/ticks, serialize IO cleanup and exercise late replies and saturated delivery explicitly.
- **Persistent IO failure leaves edits without a newer recovery snapshot** → Report it once per identical failure episode, retain any prior copy and keep retrying while protection is needed. This does not change the existing durability contract to cover power-loss guarantees beyond the current sync/rename protocol.
- **Column clipping diverges from future proportional/grapheme layout** → Follow the current editor column model and keep text/run boundaries valid; a future layout-model change must update this calculation too.
- **Cross-host numbers suggest misleading speedups** → Accept behavior and work bounds first; compare matched local measurements and retain their environment metadata.

## Migration Plan

No stored data migration is needed. Internal recovery operation/message changes must land with all callers and tests in the same coherent change. Implement the independent fixes in reviewable steps, with benchmark repair first and syntax reuse before final Swiper performance validation. Existing user commands and settings remain compatible. Reverting an implementation step must also revert its corresponding protocol callers; existing recovery files stay readable either way.

Before completion, update `docs/editor.md` for visibility-gated message rendering, acknowledged auto-save/retry and bounded result presentation, and `docs/performance.md` for the repaired harness and measured results. Keep the supplied baseline files intact and add new measurement artifacts for new runs.

### Structural acceptance checklist

- [x] All six findings have implementation and focused validation coverage; no omitted scan candidate is silently included.
- [x] Scope reuse handles explicit text edits and keeps snapshots isolated; definition queries are batched.
- [x] Preview materializes/converts only displayed intersections without losing the first-match identity or full-line semantics.
- [x] Hidden message streams retain dirty state; shown panes flush before display; block finalization remains unconditional.
- [x] Recovery requests distinguish pending from persisted state, retry without input, and survive edits, replacement recovery and cleanup races.
- [x] Worker completion delivery cannot block shutdown; GUI, headless, harness and service tests use the same protocol.
- [x] Swiper text and syntax ranges are bounded before allocation/painting and passed to GPUI with valid byte runs; navigation is independent of presentation.
- [x] Benchmark corrections preserve existing fixture and sampling definitions; new cases have distinct names and retained raw results.
- [ ] Core remains independent of GPUI/Tokio, existing macOS/Linux checks pass, and no dependencies, settings or recovery format are added.

Evidence: [scenario reconciliation and platform status](../../../docs/measurements/2026-11-01-hot-paths/validation.md). The final platform checkbox awaits the draft PR CI matrix.
