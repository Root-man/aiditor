# Validation

## Boundary migration inventory (task 1.3)

This inventory assigns the existing harness behavior before removal in task 13.1. Destinations below are migration targets, not claims that migration is complete. Source paths are under `crates/aiditor-core/src`; service paths are under `crates/aiditor-services`.

| Harness behavior | Existing test coverage | Filesystem contract destination | Pure core replacement |
| --- | --- | --- | --- |
| Save | `visiting.rs`: open/edit/save, conflict, BOM/CRLF, missing file, failed save; `watching.rs`: keep-mine, deleted-file recreation; `autosave.rs`: recovered-file save/conflict | `src/files.rs` write tests and `tests/boundaries.rs`; overlapping writes in `tests/file_sync.rs` | Explicit `Saved` success/error with buffer and tick; retain quit/save-some/kill-workspace queues, modified state, recovery cleanup and edit-during-save assertions |
| WriteProject | `project_commands.rs`: `create_writes_a_file_that_loads_back`, `edits_rewrite_the_file_and_keep_its_comments`, `ad_hoc_projects_are_edited_in_memory_then_saved` | `src/projects.rs` and `tests/boundaries.rs`: TOML round trip, comments, validation, conflict, recreation and create collision | Explicit `ProjectWritten`; command arguments, ownership and loaded state stay in core tests |
| Index | `project_commands.rs`: root files/counts, missing root, first-use finder; `watching.rs`: counts, limits, reloads, scope changes, creates/deletes, shared roots | `src/files.rs`: ignore/link/order/inventory fixtures; `tests/file_sync.rs`: notification-driven refresh | Explicit `Indexed` with chosen generation/scope/counts; delayed/reordered replies cover coalescing, announcements and eligibility |
| Check | `watching.rs`: deleted files, conflicts, rescan, focus, cross-workspace changes | `src/files.rs::tests::check_hashes_files_and_reports_gone_ones`; `tests/file_sync.rs` | Explicit `Checked` hashes/missing entries and `FilesChanged` events; retain buffer/echo/prompt decisions |
| Open | `visiting.rs`: existing/missing/binary/non-UTF8 files and directory refusal; project finder and grep acceptance | `src/files.rs` read/run tests and `tests/boundaries.rs` | Explicit `Opened`; fixture setup uses prepared bytes with `Core::open`; workspace ownership, buffer reuse, marks and navigation stay in core |
| Preview | `visiting.rs`: finder preview; `project_commands.rs`: hidden/visited/stale preview; `grep.rs`: selected hit and far-hit restoration | `tests/boundaries.rs`: production read/decode/preparation failures and BOM/CRLF | Explicit `Previewed`; retain hidden-buffer lifetime, selection, point, restoration, cancellation and session/request checks |
| Revert | `visiting.rs`: marker/line/undo/read-only/stale revert, line endings and errors; `watching.rs`: clean-copy synchronization and take-disk | `src/files.rs::tests::run_diffs_a_revert_against_the_file`; `tests/file_sync.rs`; production error/encoding fixtures in `tests/boundaries.rs` | Explicit `Reverted` prepared by pure `buffer::revert` fixtures; preserve markers, undo, stale ticks and conflict decisions |
| Diff | `visiting.rs`: diff text, no change, killed/other-workspace reply; `watching.rs`: compare; `autosave.rs`: recovered comparison | `src/files.rs::tests::run_diffs_the_file_against_the_buffer`; `tests/boundaries.rs` read failures | Explicit `Diffed`; retain display placement, ownership, read-only state and prompt behavior |
| ListDir | `visiting.rs`: directory completion, relative names, listing errors, stale listings and directory suffixes | `src/files.rs::tests::lists_sorted_with_directories_marked` and `run_replies_with_path_prefixed_errors` | Explicit `Listed`; retain TAB/RET/M-RET, typed paths, history, stale directory and completion selection behavior |
| Grep | `grep.rs`: search/move/jump, labels, errors, debounce, stale batches, cancellation, selected/far-hit previews; `jump.rs`: grep jump history | `src/grep.rs`: production regex, smart case, ignore, binary, ordering, Unicode, limits, batching and cancellation; `tests/boundaries.rs` for integrated hits | Explicit `Grepped` batches and collected `GrepOp`; retain labels, debounce, search ownership, jumps and prompt cap |
| Settings/keymap loading | `bindings.rs`: reload, invalid/unreadable files, removed bindings | Shared production configuration loader tests during task 9 | Explicit accepted reload bundle; pure settings/keymap parsing tests stay in core |

All other tests in `editing`, `motion`, `mark`, `display`, `frame`, `prefix`, `minibuffer`, `jump`, `workspace`, `autosave`, `bindings`, and `lib` exercise core state even when their setup calls `harness::on`, `open`, or constructs temporary files. Replace that setup with in-memory text/paths and explicit replies. Filesystem assertions embedded in those tests follow the operation rows above; preserve their core assertions. `walk_files`, `read`, and the harness regex grep are implementations to delete after these replacements, not shared helpers to move into production.

## Executed checks

- Before changes: `cargo test -p aiditor-core --lib --quiet`: 349 passed.
- Task 1.1: controlled saves answered in reverse order across workspaces: passed. Effects are collected without running filesystem handlers.
- Task 1.2: `cargo test -p aiditor-services --test boundaries --quiet`: passed. Production index, visit and save replies are held; save acknowledgement is delivered after switching workspace.

The editor adds a final newline during save. Test expectations retain that behavior.

- Stage 2: 355 core tests, 21 frontend tests, 45 service unit tests, the boundary test, crash-recovery test and 4 native file-sync integration tests passed. Core/services clippy passed with warnings denied.
- Native watcher tests require execution outside the filesystem sandbox on this macOS host: six tests timed out inside it; all 11 watcher tests passed outside it. All services tests above ran outside the sandbox.
- Project reconciliation tests cover rename, invalid/missing/reappearing definitions, duplicate names, file renames, last-workspace cleanup, retained index ownership, selected project across prompts, ad-hoc save identity and superseded write refresh.

## Previous stop point: tasks 1.1–3.3

Stages 1–3 are complete (11/53 tasks). Stop after window-configuration consolidation; stage 4 and later stages are not started. The change is not ready to archive.

- Final current-tree macOS checks: `cargo fmt --all --check`, `cargo clippy --all-targets -- -D warnings`, and `cargo test --quiet` passed. Tests: 21 frontend + 356 core + 45 services + 1 boundary + 1 crash recovery + 4 file sync = 428 passed, zero failed. Native watcher/file-sync tests ran outside the filesystem sandbox.
- Structural check: Core owns one `Windows`; inactive workspaces store `Option<Windows>`; switching swaps the whole value; active and saved configurations use the same killed-buffer replacement helper.
- Behavioral coverage: existing window/display/workspace, marker release, preview cancellation and cross-workspace buffer tests pass; the new switching test preserves selected window, splits, point, mark, mark ring and scroll, and drops an inactive window click. No snapshot updates were required.
- Stage 4 was not implemented: `FileOp::Open` is still path-based and the deferred grep jump remains unchanged.
- Linux and manual GUI flows were not run. They remain part of the unfinished whole-change verification, not claims of completion at this stop point.


## Tasks 4.1–6.3: implementation and evidence

- Visit reads carry workspace, window and request identity through the shared production runner. Navigation changes permanently invalidate pending reads; ordinary edits preserve them. Deferred grep coordinates belong to that visit. Core tests cover successful ownership, binary validation, errors after cancellation, switching away and back, buffer/window/workspace deletion, new prompts, repeated same-path requests and Unicode grep columns. GPUI and headless already delegate all file operations to `files::run`.
- Buffer saves and project writes share one services gate around expectation checking and the complete write. Typed create/update expectations distinguish conflicts, occupied paths and I/O failures. Real-service tests cover simultaneous same-path, symlink and hard-link saves; a project write racing a buffer write; a queued create collision; deleted project recreation with comments; unreadable files; permission and link preservation; exclusive temp collisions and ownership-limited cleanup.
- `tests/boundaries.rs` holds production operations/replies to test two workspace saves and save-some/quit/workspace-kill completion. Successful saves advance the requested action, conflicts preserve edits and the editor/workspace, and later edits remain modified. Save-some is the existing shared confirmation flow, exercised by saving one buffer while declining another and declining exit; no new command was added.
- Project-write filesystem behavior was removed from the core harness. Its successful replies now prepare only parsed in-memory definitions; conflict tests deliver explicit failures. Core project tests assert prepared/accepted text and unchanged state; disk recreation/conflict/collision assertions run in services. Other filesystem substitutes remain for task 13.1.
- `file_index.rs` owns readiness, requested and accepted scopes, request generations, one active/one latest queued walk, counts, missed paths and announcements. Controlled-reply tests cover burst requests, newest queued scope/patterns, duplicate/late replies, release, one final announcement, same-scope watch/mode-line continuity, partial errors and changed-scope recovery. A scope changed away and back also waits for fresh counts.
- The new queued index reply exposed a frontend test teardown race: GPUI could drain a file reply after the test's Tokio runtime guard had dropped. The two tests owning a runtime now release their editor/window and drain GPUI before leaving it. Production runtime handling is unchanged.

### Verification history and limits

- The first full macOS run passed 442 tests before the scope-return regression test was added. A later run exposed the frontend teardown failure above. An intermediate whole-app shutdown attempt then segfaulted in the frontend test and was removed; the normal window-removal teardown passes the isolated binding test. Failed attempts are not counted as passing runs.
- Native watcher tests timed out in the filesystem sandbox; the same 11 tests passed outside it. Native verification below uses unsandboxed execution.
- Linux and manual GUI flows were not executed. They remain tasks 7.6, 14.2 and 14.3, not claims of completion here. The process-local write gate does not serialize external programs, and the preserved in-place fallback is not crash-atomic.
- The requested batch ends at task 6.3: 22/53 complete; task 6.4 is next. Later watcher, preview, configuration, finder, grep and Swiper changes remain pending.

### Final macOS checks for this batch

- `cargo test -p aiditor-core -p aiditor-services --quiet`: 364 core, 48 services, 5 boundary, 1 crash-recovery and 4 file-sync tests passed (422 total).
- `cargo test -p aiditor --bin aiditor --quiet`: all 21 frontend tests passed after the window teardown correction. Combined final workspace coverage: 443 passed, zero failed.
- `cargo fmt --all --check` and `cargo clippy --all-targets -- -D warnings`: passed. Cargo reports existing future-incompatibility notices for `block` and `proc-macro-error2`.
- `cargo tree -p aiditor-core`: no GPUI, Tokio or services dependency; manifest/config scan: no aborting panic profile; `git diff --check`: passed.
- `openspec validate refactor-m1-projects-workspaces --strict`: passed. Apply progress reports 22/53 complete, 31 remaining, with 6.4 next.

## Tasks 6.4–8.2: implementation and evidence

- Completed the next 11 tasks: sorted index lookups, accepted kept-directory snapshots, ignore-policy refresh, watcher recovery and acknowledged registration state/retries/fallback, native platform recovery, and bounded prepared previews. Progress is 33/53; task 8.3 is next. Later configuration, finder, grep, Swiper and whole-change verification tasks remain pending.
- The sorted lookup fixture uses 100,000 sorted `dirNNNNNN/file` paths and 100 directory events, in the ordinary unoptimized test profile. It compares 4,950,100 linear path checks with 1,900 sorted checks. Both paths return the same answers. This measures comparison work, not wall-clock latency. Core tests also cover `app` versus `app2`, exact creates, empty directories, deletes and missed-path suppression.
- Production walk fixtures cover ignored trees, empty directories, root inclusion, directory counts, shared roots and zero watch limits. Files and absolute directory snapshots come from the same walk; aggregate directory snapshots are sorted. Shared immutable snapshots avoid copying every directory during normal input handling. A subprocess-isolated global git-excludes fixture verifies both explicit indexing and configuration reload; reported excluded ignore files refresh filtering while unwatched projects retain on-demand behavior.
- Accepted snapshots reach watch reconciliation even for equal scopes. Device/inode checks and removal/rename invalidation repair registrations and request index recovery for replacement trees. Failure-injection tests cover add/remove/commit failures, uncertain outcomes, unaffected registrations, changed-failure suppression, capped deadlines and cancelling released coverage. The service loop recovers from initialization and registration failures without incoming filesystem events. Visited fallback depends on acknowledged root coverage; fresh counts can release oversized root coverage without losing visited-file synchronization.
- Native `tests/file_sync.rs` exercises lost-directory recovery, an excluded `.ignore` becoming inclusive, replacement with a new nested directory, and size-limit fallback. The recovery test waits for events to settle, then requires a later independent write at the exact file path. These tests are in the ordinary `cargo test` targets already selected by `.github/workflows/ci.yml` on macOS and Linux. Both native platforms were exercised locally; no branch was pushed and no remote GitHub CI run was triggered.
- Preview reads carry prompt-session and request identity, with one active and one replaceable pending request across closure/reopening. Core tests cover rapid selection, returning to an active file, closed queues and old same-path sessions. Services validate/decode UTF-8, BOM/CRLF, binary, missing, invalid and unreadable files, prepare Rope text on the worker and omit the unused content hash. A channel-held production worker verifies query input and cancellation complete before the worker resumes. GPUI and headless keep dispatching through `files::run`; buffer installation takes the prepared Rope without re-decoding it.

### Accepted limitation

The user explicitly authorized using the backend's public acknowledgments. With notify 8.2.0 on macOS, `FsEventPathsMut::commit`, `watch_inner` and `unwatch_inner` discard some `run()` startup errors; the native `FSEventStreamStart` result is also ignored internally. "Confirmed" therefore means acknowledged by that public API, not independently observed OS coverage. Hidden failures cannot activate retries or reporting. The limitation is recorded in `design.md`, `docs/editor.md` and at the backend commit boundary. No dependency patch or new dependency was introduced.

### Verification and corrections

- Final macOS `cargo test --quiet`: all 460 tests passed in one complete workspace run.
- macOS: 370 core tests and 69 services/integration tests passed (53 services unit tests, 9 boundary tests, 1 recovery test, 6 native file-sync tests). Native tests run outside the sandbox because sandboxed FSEvents tests timed out. The 21 frontend tests also passed.
- Linux: the same 439 core/services/integration tests passed using the installed `rust:1.99` Docker image, with native inotify fixtures inside the container filesystem. Tests run as UID/GID 1000. The first root-user attempt failed an existing permission-denial test because root can read mode-000 files; the non-root rerun passed. This is local native Linux verification, not a remote CI result or a Linux GUI test.
- During development, focused checks caught and corrected early fallback removal, a held-worker fixture missing its initial core tick, a status assertion that overlooked existing echo aggregation, and replacement coverage needing a fresh index. Those failed attempts are not counted as successful checks.
- Formatting, all-target clippy, the core dependency boundary, panic-profile scan, and strict OpenSpec validation passed. Existing Cargo future-incompatibility notices for `block` and `proc-macro-error2` remain.
- Manual interactive GUI flows and Linux frontend checks were not run; they remain part of tasks 14.2–14.3. One very large preview can still delay the next read and consumes full-file memory; this work bounds request count rather than file size.

## Tasks 8.3–12.1: implementation and evidence

Ten tasks completed in this batch: 8.3, 9.1–9.4, 10.1–10.2, 11.1–11.2 and 12.1. Progress is 43/53. Task 12.2 is next; the Swiper optimization itself is not part of this batch.

- Prepared previews already install through the existing ownership/restoration paths. New regression coverage verifies a fresh ordinary read after acceptance, hidden-buffer release on clicks/workspace switches, and same-file grep position changes with the same prepared buffer. Existing one-hidden-buffer, visited-buffer reuse and cancellation tests pass.
- Runtime configuration reads now run on the blocking pool through the shared services loader. One numbered settings/keymap/project bundle reaches the core; accepted components apply in that order with independent fallback. One active request and the latest requested number coalesce repeats and discard obsolete/duplicate replies. A project-write success advances the revision captured by reloads, forcing a fresh bundle after a conflicting snapshot; a superseded write uses the same reload path.
- Core tests hold reload replies while typing and verify coalescing/fallback. A channel-held production loader worker also allows input before release. Real project reads/writes exercise both reload-before-write and write-before-reload reply delivery. Frontend tests verify asynchronous reload application and typing; headless tests exercise omitted configuration alongside production save/search/jump behavior. Startup keeps its synchronous reads through the shared loader helpers.
- Each finder captures root paths and their unique labels. Preview, candidate acceptance and typed acceptance use that mapping until close. Initial candidates join only a waiting finder with compatible roots; a finder with an existing snapshot stays unchanged. Newly opened finders wait for compatible results when old indexed roots differ from the current definition. Tests cover initial typed prefixes, duplicate labels, reordering, reload while open, obsolete initial results and reopening.
- Grep uses one flush path from matches, file completion and search completion. A controlled clock proves two intermediate batches arrive during a single four-line matching scan. Production tests retain multi-root/file/line order, Unicode columns, search IDs, cancellation and exact zero/small limits, and fill the real core prompt to its 10,000-hit cap from a 10,005-line file. Deadline tests no longer depend on sleeps.

### Swiper baseline (task 12.1)

Reproduce with `cargo test -p aiditor-core prompt_and_query_baseline -- --ignored --nocapture`. This is an explicitly ignored measurement test, run separately from ordinary checks; it adds no dependency and no production optimization.

Environment: macOS arm64, Rust 1.99.0 (`b940084d7`, 2026-09-28), ordinary unoptimized test profile with debug assertions, dependency optimization level 3 as configured in the workspace. Seven samples per fixture; medians below are milliseconds. Fixture creation and `Core::open` are outside the timed region. Prompt opening handles the actual `C-s`; each query replaces the previous prompt text through `Msg::Insert`, executing actual filtering and preview. Frame assertions are outside timing. These are core handling costs, not GUI rendering or input-latency guarantees.

Fixture: lines numbered from zero with `format!("{i:06} alpha needle value {kind} été\n")`, where `kind` is `skip` every fifth line and `keep` otherwise; each line ends with a real newline. Small: 1,000 lines / 37,000 UTF-8 bytes. Large: 100,000 lines / 3,700,000 UTF-8 bytes. Queries run in the table's order for every sample.

| Operation | 1,000 lines | 100,000 lines |
| --- | ---: | ---: |
| Open prompt | 0.295 | 26.358 |
| `needle` | 0.212 | 9.103 |
| `needle !skip` | 0.381 | 13.592 |
| `NEEDLE` | 0.067 | 4.352 |
| `alpha value` | 0.281 | 9.651 |
| `(` (literal fallback) | 0.055 | 4.213 |
| `needle\ value` (escaped space) | 0.236 | 9.227 |

Swiper still scans all lines per query and recompiles for filtering/preview/acceptance. Tasks 12.2–12.4 will change and remeasure that behavior against this baseline.

### Verification scope

Final macOS `cargo test --quiet`: 474 passed, zero failed (21 frontend, 379 core, 57 services unit, 1 headless, 9 boundary, 1 crash-recovery and 6 native file-sync tests). The single ignored Swiper measurement test passed separately with the results above. Native watcher tests ran outside the filesystem sandbox.

Formatting, all-target clippy with warnings denied, the core dependency boundary, the panic-profile scan and strict OpenSpec validation pass. Existing Cargo future-incompatibility notices for `block` and `proc-macro-error2` remain. During implementation, compiler checks caught and corrected test-only use of `find` on a vector and a private minibuffer accessor before passing reruns.

Linux and manual interactive GUI flows were not run in this batch; whole-change cross-platform and integrated GUI checks remain tasks 14.2–14.3. No changes were pushed or published.
