# Design

## Context

See `proposal.md` for the motivation and numbered opportunity inventory. This design is grounded in the post-M1 tree at `aba5495`, after the same-scope watch-continuity fix in `95af7df`.

- `aiditor-core/src/lib.rs` applies path-only `Opened` replies to the current window; `grep.rs` separately keeps a path-only deferred jump. `minibuffer.rs` also identifies pending preview reads by path alone.
- `aiditor/src/editor.rs` spawns each file effect independently. `aiditor-services/src/files.rs` checks the disk before writing and uses a process-wide, target-derived temporary name. `projects.rs` calls the same writer but rewrites error strings.
- `project_commands.rs` owns `FileIndex`, while `watching.rs` mutates its state and scans sorted file vectors linearly. Every invalidation starts another full walk.
- `aiditor-services/src/watcher.rs` caches kept directories; a rescan forwards a flag without rebuilding them. Registration failures are logged, then the requested set becomes the recorded installed set regardless of success.
- Runtime reload reads settings, keymaps and project files synchronously in the frontend. Workspaces and indexes refer to projects by name. Active window fields duplicate `workspace::Windows`.
- The core harness reimplements filesystem operations with different ignore, write and error behavior; service integration tests generally execute file effects immediately. Existing native watcher and frontend tests remain useful but do not cover all completion orderings.

Accepted M1 decisions are constraints: a deleted visited or project file can be recreated; workspace switching closes the minibuffer; duplicate project names currently resolve to the first loaded match; the index is in memory; quiet reindexing does not continuously append into an open finder. The prior directory-recreation and missing-project-reload limitations are deliberately addressed here. Existing project TOML and command keys stay compatible.

## Goals / Non-Goals

**Goals:** Keep ownership and freshness checks in the single core state machine, filesystem work in services, and frontend adapters small. Make failure and completion order controllable in tests. Cover every numbered opportunity in one change, with independently testable stages.

**Non-Goals:** A general job scheduler, a generic repository/service abstraction, new dependencies, persisted workspace or project IDs, cross-process file locking, a file-size cutoff, a fully incremental index, background Swiper filtering, or new project-name disambiguation UI. Performance work removes known duplicated/unbounded work first; measurements describe remaining limits without claiming a latency guarantee.

## Decisions

### D1. Explicit file-visit intent (#1)

Issue a monotonically increasing visit token with the originating workspace and ordinary window IDs. Keep any grep destination on that intent, and echo the token in the service reply. All asynchronous visit callers use this path: ordinary and project finders, project-file opening, and grep acceptance. Existing-buffer visits remain immediate and invalidate any earlier pending visit for that window.

A visit remains valid only while its origin is live, selected and still represents the same navigation intent. Switching workspace or ordinary window, deleting the origin, selecting another buffer, starting another visit, or opening another prompt invalidates it permanently, even if focus later returns. Ordinary edits do not invalidate a request. A late invalid reply, including its error or grep location, is discarded before creating a buffer or changing a window. A valid reply uses the issuing workspace's normal visit/reuse rules. Initial command-line loading keeps its existing startup policy.

This makes abandoning a pending navigation explicit and avoids a late completion taking over a newer prompt. Alternatives were mutating inactive windows or queuing results until focus returns; both add deferred UI behavior for a command the user has already moved past. Separate request IDs, rather than paths, also distinguish two requests for the same file.

### D2. One small write critical section and typed failures (#2, #11)

Put a process-local write gate in the services layer around the disk expectation check and the complete write/replace operation. Both buffer saves and project writes enter it, including project creation's absent-file check. Start with one gate for editor file/project writes: it covers symlink and hard-link aliases without a lock-key registry whose identity changes during atomic replacement. Reads, indexing and grep remain independent. Encoding and project parsing/validation stay outside the gate where possible.

Use an explicit write expectation to distinguish create-only operations from updates that permit recreation of a missing file. Return a small typed failure distinguishing disk conflict, already-exists and I/O failure; format buffer-save and project-reload guidance at their boundaries. Remove substring replacement. Avoid recursive locking when a project write calls the shared low-level writer.

Create a unique, exclusive temporary file per replacement, using a process identifier plus monotonically increasing suffix and retry on collision. Never remove another operation's temporary file. Cleanup owns only its successfully created temp. Keep permission preservation, symlink targets, the hard-link/in-place fallback, sync-before-rename and buffer-tick checks.

Alternatives: per-target locking adds identity, alias and cleanup machinery; replacing only the temporary filename leaves the conflict-check race open. A single gate is the smallest correct initial design for infrequent local editor saves. It does not make external programs participate in the critical section or make in-place fallback writes crash-atomic.

### D3. Stable project handles and explicit reload reconciliation (#13)

Allocate a session-local `ProjectId` for each loaded entry; use it for workspace bindings, index ownership and asynchronous project operations. Keep display names as data, not keys. Saved entries match reload results by their existing absolute project-file path, preserving their ID even when the TOML `name` changes. Ad-hoc entries retain IDs; saving one attaches a file to the same ID. Do not canonicalize project root paths or add IDs to TOML.

Reconcile successful reload results first, retain ad-hoc entries, and retain the last valid saved definition for any live workspace whose file is absent or invalid. A retained entry keeps its original file path and last-loaded text, so editing a deleted file can recreate it while editing an invalid or externally changed file still conflicts. Report load errors through the existing flow. Drop removed saved entries once unbound; release their indexes and disregard their late replies. Reappearance of the same path updates the retained ID. Renaming a project file itself creates a new identity and leaves the old bound definition until it is unbound; do not guess renames from names or roots.

Keep existing first-match name resolution for commands, with a deterministic order: pre-existing ad-hoc entries, successfully loaded saved files in file-name order, then retained bound saved entries in their prior order. Existing workspace bindings never change because another entry has the same name. A name change updates project labels but does not rename workspaces or scratch buffers. Pending operations retain the chosen ID rather than resolving their name again.

Avoid replaying an older project-write completion over a newer reconciliation: tag writes with the entry revision and write intent, apply only to that still-current revision, and refresh that file through the normal loader after a successful superseded write. The disk write still checks its expected content. This prevents the background reload introduced in D7 from restoring an older definition in memory.

Alternatives: retaining names alone leaves duplicate/rename aliasing; persisted UUIDs require a file-format migration; rejecting duplicate names changes an accepted M1 policy. The chosen handles are internal and session-scoped.

### D4. One owner for index state and bounded rebuilds (#7, #8)

Move index state and transitions to a dedicated core module. Key it by `ProjectId`. Its interface owns requesting a walk, applying a reply, invalidating from paths, observing the usable snapshot/counts, and releasing a project. Callers do not mutate readiness, pending generation, scope, missed paths or loud-request state separately.

Keep at most one walk in flight and one coalesced pending refresh per project. Each refresh updates the latest requested scope/generation; it does not spawn another worker immediately. When the active walk finishes, discard a superseded answer and start the latest queued refresh. Carry a user-requested completion announcement forward through quiet invalidations so it is emitted once for the final accepted answer. An answer for a released ID is discarded.

Keep a last accepted snapshot separate from pending scope. A same-scope refresh preserves counts, watches and mode-line eligibility until an accepted answer arrives; changed roots or project ignore patterns suspend root eligibility until their result arrives. Use binary search for exact relative paths and a lower-bound lookup followed by component-aware `starts_with` for descendants; `app` must not match `app2`. Preserve missed-path suppression, resetting it when scope or ignore-file rules change so old exclusions do not hide later invalidations.

Have the shared service walk return kept directories alongside sorted files, deriving the directory count from that same result. This supplies authoritative recovery coverage to D5 without a second independently filtered full walk. Retain partial-root error reporting and conservative watch eligibility after errors.

Alternatives: cancelling every active disk walk adds cancellation plumbing; continuously spawning walks wastes work even when replies are discarded; a per-path incremental index adds substantially more consistency logic. Coalescing bounds work without those additions. Under a continuous event stream freshness can wait for a quiet enough walk, but typing and prior usable snapshots remain available.

### D5. Watch coverage and confirmed registrations are distinct (#3, #4)

Treat the eligible index snapshot's kept directories as desired project coverage, and the service's successful backend registrations as confirmed coverage. Preserve the existing macOS recursive-root/Linux non-recursive-directory topology, shared-root union and parent-directory fallback for visited files.

Lost events request both buffer checks and a recovery walk. Changes to `.gitignore` or `.ignore` under a managed root invalidate its filtering even when the ignore file itself is filtered; explicit reload/reindex also rereads ignore files and global excludes. Such refreshes clear missed-path suppression. Do not add a new global-exclude-file watcher. Publish the fresh directory snapshot and recovery information to the watcher even when root paths and project patterns compare equal. Fresh counts still decide `max_watched_files` eligibility; same-scope recovery keeps the old eligible watches until that decision arrives.

Reconcile native registrations against the recovered snapshot, verifying the identity of directories behind existing paths. Record directory identity in the service using the metadata already available on the supported Unix platforms. A deleted/recreated directory at the same path, or a path the backend reports invalidated, needs a fresh registration. Preserve registrations whose path, mode and directory identity remain valid. A normal same-scope walk must not tear down every watch.

Update confirmed state only for acknowledged registrations/removals. A failed or uncertain batch commit remains unconfirmed and is retried/reconciled; it must not copy desired state into confirmed state. Use the backend's supported operation semantics, with a narrow test seam for add/remove/commit failures. When the backend cannot reveal the outcome of a failed batch, treat its affected registrations as uncertain and conservatively re-establish them. A total backend failure permits a rebuild followed by a rescan.

Retry pending failures on a service deadline independent of incoming filesystem events, with capped backoff (1, 2, 4, 8, 16, then 30 seconds) and an immediate attempt after relevant desired coverage changes. Surface the first failure and changed failures through the core's echo/`*Messages*` route; suppress identical repeats and report recovery once. Cancel retries when coverage is no longer requested. Keep visited-file fallback for paths without confirmed root coverage, including failed root registrations.

Backend acknowledgment limit (accepted during implementation): confirmed means acknowledged by `notify`'s public API. In notify 8.2.0 on macOS, batch commit and watch/unwatch discard stream startup errors, and the FSEvents stream-start result is ignored internally. These unreported failures cannot trigger our retry/reporting policy. This change does not patch or replace the backend; reported failures and uncertain public batch outcomes remain recoverable as specified.

Alternatives: logging alone hides permanent lost coverage; periodically rewatching everything adds gaps and churn; relying on set equality cannot detect replaced directories. Confirmed state plus targeted reconciliation addresses the observed failure modes. Do not redefine `[unwatched]`, which currently expresses project eligibility; registration errors use explicit messages.

### D6. Preview session identity and coalescing (#5)

Give each previewing prompt a session ID and each selected file request a revision. Keep one active preview read and one replaceable latest pending request across prompt sessions; closing a prompt invalidates both its result and queued work. A new prompt can replace the pending request while the old read finishes. Repeated selection of the same file within a session reuses its active request or hidden buffer, while grep positions within that file follow the newest selected hit.

Perform file read, binary/UTF-8 validation, BOM/line-ending decoding and Rope preparation in the worker. Pass prepared immutable text to the core so installation does not copy/decode the whole file on the UI thread. Preserve the core/services dependency direction: the transferable data type lives with the core protocol. Do not compute an unused preview content hash. A stale result is released and only the latest pending request starts; it cannot install into another prompt that happens to select the same path.

Keep the existing hidden read-only buffer, origin restoration, visited-buffer reuse and fresh ordinary read on `RET`. An OS read already in progress can finish; obsolete queued reads never start. No byte-size limit or new debounce setting is introduced. This bounds concurrent/request-retained work, not the size of one file.

Alternatives: path-only checks allow a previous prompt's result into a new one; a debounce alone still allows overlapping reads; truncating previews changes file-display behavior and is outside this refactoring.

### D7. Background configuration reloads with ordered application (#9)

Use one background reload operation to read settings, keymaps and projects, including filesystem validation, then return one request-numbered bundle. The core applies accepted bundle components in the current order: settings, keymaps, projects. Keep each component's existing validation/fallback behavior; this is not an all-or-nothing configuration transaction.

Allow one active reload plus one pending repeat. If a newer request exists, discard the older bundle and execute the latest request. Typing and redraws continue while reads run. Startup loading may stay synchronous before the window exists. Preserve a newer successful project edit when an older reload snapshot arrives by checking the project revision captured at reload start and queueing a fresh load on conflict. Update GPUI and headless adapters and tests together. The existing headless runner can continue to omit native watching and user-configuration reload; update its exhaustive effect handling and retain its shared execution of supported file/search operations.

Alternatives: moving three reads to independent jobs allows mixed generations; keeping I/O in `Editor::handle` blocks interaction; a general scheduler is unnecessary for one operation.

### D8. Timely grep batches and stable finder resolution (#10, #12)

Move the existing grep batch deadline check into match processing, sharing the flush path with file-end and final completion. Preserve root/file/line order, search numbers, exact hit cap, cancellation checks before sends and the final done marker. Use controllable time in batching tests; a timing assertion must not rely on a large file happening to take 50 ms. No timer thread is needed to interrupt a blocked read or a match-free region.

Capture the root-name-to-path mapping with each project finder session, using its displayed index snapshot when available and current project roots when the initial index is pending. Use that same mapping for preview, accepted candidates and typed names; a reload cannot reinterpret a displayed `lib<2>/x` against different roots. If the initial walk completes for a superseded scope, leave the prompt's mapping intact and do not inject mismatched candidates. A reopened finder uses the latest definition. Typed prefixes therefore work before the first result without changing candidate snapshot semantics.

Alternatives: resolving names from whatever project definition is current can open the wrong root; waiting for the index makes new-file creation unnecessarily depend on a walk. Grep acceptance uses D1 rather than a separate path-based jump slot.

### D9. Narrow Swiper cleanup before larger performance work (#14)

Represent the parsed/compiled query once per prompt text revision, and use it for filtering, preview target calculation and final acceptance. Keep its lifetime within the prompt/acceptance transition rather than a global cache. Retain an immutable Rope snapshot and line references where useful; create formatted line labels at the existing completion boundary, avoiding repeated copies of all text during filtering. Adapt only the filter/preview seam required by Swiper, preserving generic fuzzy completion.

Measure prompt opening and repeated query changes separately on reproducible small and large multiline fixtures before and after the change. Preserve literal fallback for invalid regex words, smart case, exclusions, escaped separators, line order, selection stability, history and mark/jump behavior. Do not move scans off-thread or add an arbitrary latency gate in this change. The remaining full scan per query is a documented limit, with measured follow-up evidence.

Alternatives: a background search protocol adds stale-result and selection coordination before measurements justify it; caching only final results adds unbounded key storage and helps little during novel typing.

### D10. Reuse the existing window configuration type (#15)

Make the active configuration a `Windows` value on `Core`, matching the saved value on inactive workspaces. Switch whole values after closing the minibuffer; preserve the current arrangement in which the active workspace has no second stored copy. Use shared iteration/replacement helpers for killing a buffer across active and saved configurations.

Keep window IDs, split trees, selected window, marker ownership/release, per-window point/mark/ring/scroll, jump history and cross-workspace buffer display unchanged. Migrate all direct field users, display construction, commands, preview origin restoration and tests in one mechanical stage. Do not introduce a generic window manager or move active ownership into shared mutable entities.

Alternatives: leaving two representations requires every new field to be manually swapped; moving all configurations into a map would expand the borrow/lookup rewrite. Reusing the existing type is the narrower consolidation, although many core callers change.

### D11. Verify at the actual dependency boundaries (#6 and all stages)

Keep core tests deterministic: expose queued effects and deliver explicit replies in arbitrary order. Move tests that claim filesystem correctness to `aiditor-services` integration tests using `files::run`, project writers, grep and watcher policy. Remove duplicated filesystem implementations from the core harness as their tests migrate; core-only command tests use explicit prepared responses, not a second implementation of ignore/writing/grep. A small services test driver can compose the core and production services without adding a reverse dependency.

Use barriers/channel handshakes for overlapping writes and delayed completions; inject watcher backend outcomes and batch time through narrow internal seams. Keep native watcher tests on both CI platforms for real registration/recovery behavior. Preserve existing scenario coverage while moving tests; do not merely delete failing substitutes. Frontend tests cover dispatch responsiveness and accepted reload bundles, while headless tests confirm compatibility of its supported effects.

Structural acceptance checklist:

- [ ] Core has no filesystem effect implementation, GPUI dependency, Tokio dependency or dependency on services.
- [ ] One shared service write path owns disk expectation checking, replacement and typed error handling.
- [ ] One index module owns generations, scope, readiness, counts, queued refresh and missed-path state.
- [ ] Project names label stable session identities; workspace/index ownership does not use names as identity.
- [ ] Active and saved window configurations use the existing common representation.
- [ ] Swiper's filter, preview and acceptance share the same compiled query revision.
- [ ] GPUI, headless and tests handle every changed effect/reply contract.

## Risks / Trade-offs

- [The unified change is broad] → stage the tasks by dependency, retain the opportunity IDs, and verify each stage before moving on.
- [The global write gate delays unrelated saves behind a slow save] → keep preparation outside it and measure multi-buffer save behavior; consider keyed locking only with evidence.
- [Other programs can write after the conflict check] → preserve current best-effort disk conflict semantics; process-local serialization makes no cross-process guarantee.
- [Native watch APIs differ in failed-batch behavior] → model uncertainty explicitly, inject failures and exercise actual macOS/Linux backends; preserve unaffected registrations whenever confirmed.
- [Recovery walks overlap further changes] → generation/coalescing rules retain one latest refresh; install only accepted coverage and keep previous same-scope eligibility until then.
- [A retained project definition is intentionally older than invalid disk text] → keep its old expected text, report the load error, and require normal conflict handling before overwriting an existing file.
- [A single very large preview still uses memory and can delay the newest request] → only one read is active and preparation is off the UI thread; a size policy remains separate work.
- [Window consolidation touches many marker-sensitive callers] → retain behavioral snapshots and test switching, preview cancellation and killed-buffer replacement across workspaces.
- [Swiper still scans all lines] → report before/after measurements and the remaining limit rather than promising constant-time typing.

## Migration Plan

No on-disk migration or dependency upgrade is required. First add controllable effect delivery and production contract coverage, then stabilize project identity and the common window representation. Follow with visit intent, writes, index ownership/coalescing, watcher recovery, preview/reload work, and the independent grep/finder/Swiper changes. Migrate the remaining harness substitutes after their production counterparts are covered.

Run targeted tests as each stage lands, then the repository's existing format, clippy, dependency-boundary and full test checks on macOS and Linux. Record the explicit navigation/reload policies and remaining performance limits in `docs/editor.md`. Rollback is a code revert; files written by the changed editor retain the existing formats. Review and merge this as one OpenSpec change; no implementation is part of the proposal workflow.
