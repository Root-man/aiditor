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

## Tasks 12.2–14.4: final batch

### Swiper measurements and scope

The exact task 12.1 fixture and command were rerun before and after this batch on the same macOS arm64 host, Rust 1.99.0, unoptimized test profile with optimized dependencies, seven samples per fixture. No background filtering, global cache, dependency or size cutoff was added. The prompt owns one compiled query revision for filtering, preview and acceptance, plus an immutable Rope snapshot; visible labels are formatted at the completion boundary. Preview/acceptance still calculate coordinates against the live origin text, preserving behavior if a watcher updates that buffer.

| Operation | 1,000 lines before / after (ms) | 100,000 lines before / after (ms) |
| --- | ---: | ---: |
| Open prompt | 0.311 / 0.134 | 23.395 / 10.341 |
| `needle` | 0.225 / 0.255 | 8.651 / 15.807 |
| `needle !skip` | 0.398 / 0.386 | 12.463 / 19.939 |
| `NEEDLE` | 0.067 / 0.174 | 3.955 / 12.442 |
| `alpha value` | 0.286 / 0.313 | 9.072 / 16.083 |
| `(` | 0.057 / 0.162 | 3.923 / 12.333 |
| `needle\ value` | 0.240 / 0.285 | 8.861 / 15.806 |

Opening avoids allocating every formatted label, but scanning Rope lines is slower per query than scanning the former preformatted strings. This is a measured trade-off, not an across-the-board speedup: the full scan remains on the core thread. Existing smart-case/ordered-word/exclusion/escape/literal-fallback, selection, trailing-newline, history, mark and jump tests pass. A new long-line Unicode test spans Rope chunks and verifies preview/acceptance and subsequent query positions. An experimental chunk scan was removed because it added code without resolving the measured query regression.

### Completed boundary migration (13.1)

The opening inventory's migration destinations are now implemented. Core's harness has no file reads/writes, directory traversal, regex grep or tempfile setup. It binds request IDs to configured read/list/index/check/save/revert/diff/grep replies; absent fixtures leave the operation collected for explicit delivery. Saved hashes are supplied by tests, never inferred by a substitute writer. Project acknowledgements only parse the core's in-memory prepared definition; no filesystem validation is simulated. Revert fixtures use the pure buffer diff function on supplied bytes. Core assertions now check emitted save text and accepted hashes/flags instead of claiming disk effects. All 379 pre-existing active core tests remain; two additional Swiper tests bring the count to 381.

| Removed harness contract | Executed production coverage |
| --- | --- |
| Save / encoding / conflict / recreation / aliases / temps | `files` write tests; `boundaries::{visits_saves_reverts_and_diffs_use_real_encoding_and_read_failures,concurrent_workspace_saves_keep_the_losing_buffers_edits,queued_saves_finish_or_cancel_quit_and_workspace_kill_without_losing_edits}` |
| WriteProject / TOML / comments / validation / conflict / creation | `projects` unit tests; `boundaries::{project_edits_recreate_deleted_files_keep_comments_and_report_typed_conflicts,project_and_buffer_writes_share_the_disk_check}` |
| Index / ignore / counts / root failures | `files` index and inventory tests; `boundaries::{real_inventory_drives_limits_shared_roots_and_ignore_refresh,explicit_index_and_reload_reread_global_excludes}` |
| Check / deletion / clean and dirty copies | `files::check_hashes_files_and_reports_gone_ones`; all native `file_sync` scenarios |
| Open / Preview / binary / UTF-8 / missing / unreadable | `files` read/run tests; `boundaries::{production_visit_save_and_index_replies_can_be_held,preview_service_prepares_text_and_reports_invalid_or_unreadable_files}` |
| Revert / Diff / errors / line endings / recovery comparison | `files::run_diffs_*`; new encoding/read-failure and recovered-conflict boundary tests; native `file_sync` tests |
| ListDir / sorted directories / errors | `files::{lists_sorted_with_directories_marked,run_replies_with_path_prefixed_errors}` |
| Grep / order / matching / Unicode / cancellation / deadlines / limits | All production `grep` tests, including controlled-clock streaming and the integrated 10,000-hit cap; supported headless search test |
| Settings / keymaps / projects | Production `configuration` loader tests, including independent failures, held input and both write/reload orderings; frontend reload test |

`file_sync.rs` now holds real operations or replies. Two barrier-released workspace saves return in reverse delivery order: one complete edit wins, the loser retains edits, and a later native notification updates the clean winner while conflicting the dirty loser. Held production reverts delivered after further edits/workspace switching update only their original unchanged buffers. Existing replacement, ignore-policy, lost-event, fallback and limit-crossing native scenarios remain.

### Structural verification and opportunity reconciliation

The design checklist was checked against the finished implementation: no filesystem calls or services/GPUI/Tokio dependency in core; one shared service write gate; private index transition fields; `ProjectId` ownership; one `Windows` type; prompt-scoped shared Swiper query; exhaustive GPUI/headless dispatch. No new dependency was added; core's unused tempfile dev dependency was removed.

| Proposal ID | Implementation and evidence |
| --- | --- |
| 1 | `VisitIntent` and deferred destination; core visiting/grep cancellation and same-path tests; held frontend/service reads |
| 2 | `files::write`, typed expectations, gate, exclusive temps; same-file/link/project races and cleanup tests |
| 3 | Accepted directory snapshots and identity reconciliation; native replacement/ignore/lost-event recovery tests |
| 4 | Desired/confirmed/uncertain registrations, deadlines and reports; injected backend failures and autonomous retry tests |
| 5 | Prompt/request identities and one active/latest preview; held production preparation plus restoration tests |
| 6 | Prepared core replies and real service contracts; completed migration table above |
| 7 | Private `FileIndex` transition owner; generation/scope/readiness tests |
| 8 | Sorted lookups and one active/latest walk; comparison fixture and coalesced-reply tests |
| 9 | Background numbered configuration bundle; ordered/coalesced component and project-write tests |
| 10 | Shared grep flush path; controlled-clock in-file batches, order/cancellation/exact caps |
| 11 | Typed project write formatting and missing-file recreation; real comments/conflict/collision tests |
| 12 | Captured finder roots/labels; pre-index typed prefixes, duplicate labels and reload tests |
| 13 | Stable project IDs and path reconciliation; missing/invalid/renamed/duplicate/retained ownership tests |
| 14 | Shared prompt query, lazy labels and Rope snapshot; behavior tests and comparable measured trade-off above |
| 15 | Common active/saved `Windows`; workspace/display/marker/preview/jump tests without snapshot changes |

`docs/editor.md` was reconciled against all nine delta specs. It distinguishes preserved M1 policies (deleted-file recreation, duplicate first match, on-demand unwatched indexes, minibuffer closure on workspace switching) from changed navigation cancellation, retained identities, ordered reloads, stable finder mapping and watcher recovery. Existing limitations remain: public notify acknowledgement cannot reveal hidden macOS stream-start failures; the gate is process-local and in-place fallback is not crash-atomic; full-file preview memory and synchronous Swiper scans remain unbounded by file size.

### Final verification (14.2–14.4)

All ten tasks in this batch are complete: 12.2–12.4, 13.1–13.3 and 14.1–14.4. Overall progress is 53/53.

| Check | macOS arm64 host | Linux isolated container |
| --- | --- | --- |
| `cargo fmt --all --check` | Passed | Passed |
| `cargo clippy --all-targets -- -D warnings` | Passed | Passed |
| Core dependency and panic-profile checks from CI | Passed | Passed |
| Full `cargo test` | 481 passed, zero failed, one ignored measurement | 481 passed, zero failed, one ignored measurement |

Both final suites ran with Rust 1.99.0 on the finished code: 22 frontend, 381 core, 57 services unit, 1 headless, 11 boundary, 1 crash-recovery and 8 native file-sync tests. The ignored measurement passed separately on macOS, as recorded above. Linux used the CI system dependencies and a non-root test user with fixtures on the container filesystem; macOS native notification tests ran outside the filesystem sandbox. These are local executions of the CI checks, not a claim that hosted GitHub CI ran. Existing dependency future-incompatibility notices remain for `proc-macro-error2` on both platforms and `block` on macOS.

The new GPUI `integrated_finders_visits_saves_and_directory_recovery` test passed on both platforms. It drives rapid finder/grep selection and acceptance, a held ordinary read across workspace switching and typing, directory replacement followed by another independent disk notification, and queued conflicting saves from two workspaces. It waits for an observed write before replacing the directory, so recovery is tested after native coverage is established. Existing GPUI reload-during-typing coverage and the held production configuration-loader test also pass. These are automated GUI event flows; manual visual rendering and physical keyboard interaction were not checked.

Final review found and corrected live-buffer match placement after a watcher revert, including a buffer shortened below the selected snapshot line. The new regression test verifies the live match position and graceful acceptance failure after shortening. Full platform suites were rerun after this correction. Strict OpenSpec validation and `git diff --check` pass. No changes were pushed or published.

## Verification follow-up

The review identified two reproducible error paths. Before the fixes, `a_current_write_error_is_reported_after_reload_changes_the_revision` failed with an empty echo area, and `a_failed_create_can_be_retried` failed because retry returned `AlreadyExists`. Both regression tests pass after the fixes.

- Given a current project write held across a reload, when its error arrives after the entry revision changes, then the reason appears in the echo area and the reloaded definition remains. Revision checks still suppress applying a superseded successful write and request fresh configuration.
- Given a create that fails after creating its file, when the user retries, then creation succeeds. A subprocess with a file-size limit reproduces a real partial write without changing limits in the main test runner. Write/sync failure removes the newly created file on a best-effort basis and preserves the original error; cleanup itself can still fail due to filesystem errors.
- Given a Swiper snapshot containing blank lines, Unicode and non-LF separators, when filtering or accepting a match, then line labels and match positions retain the existing LF-only semantics. Existing empty/trailing-newline, chunk-spanning, live-revert, history and selection tests remain applicable.

The flat snapshot uses one prompt-owned string plus line byte ranges; line ends exclude LF from the outset, avoiding per-query Rope traversal and trimming. This adds a full text copy for the prompt, with no global cache or background filtering. The full scan remains synchronous. Design D9 and `docs/editor.md` now describe this representation.

### Swiper remeasurement

Same task 12.1 command, fixture, host, Rust 1.99.0 and debug/dependency optimization settings; seven samples and median milliseconds. The before column was measured immediately before this follow-up's implementation, and the after column uses the flat snapshot with precomputed line ranges.

| Operation | 1,000 lines before / after (ms) | 100,000 lines before / after (ms) |
| --- | ---: | ---: |
| Open prompt | 0.120 / 0.112 | 10.232 / 4.053 |
| `needle` | 0.219 / 0.251 | 15.623 / 7.248 |
| `needle !skip` | 0.339 / 0.478 | 19.901 / 11.369 |
| `NEEDLE` | 0.156 / 0.133 | 12.291 / 4.111 |
| `alpha value` | 0.276 / 0.351 | 16.150 / 7.633 |
| `(` | 0.141 / 0.099 | 12.338 / 4.140 |
| `needle\ value` | 0.256 / 0.306 | 15.795 / 7.439 |

Large-buffer query costs are now approximately the pre-refactor range (3.923–12.463 ms), with most queries faster and the two no-match cases slightly slower. Small-buffer timings are mixed at sub-millisecond scale. These measurements do not establish a GUI latency guarantee.

Hosted CI for the original PR head `ea0b482` passed on both macOS and Ubuntu: [run 39916293828](https://github.com/Root-man/aiditor/actions/runs/39916293828). This closes the earlier hosted-CI evidence gap for that commit; it does not verify the follow-up until a new run completes. Manual visual/physical-keyboard checks remain unperformed; task 14.3 was exercised through automated GPUI flows.

Follow-up local macOS checks passed: 484 tests (22 frontend, 383 core, 58 services unit, 1 headless, 11 boundary, 1 crash recovery, 8 native file sync), zero failed, with the measurement test ignored in the ordinary suite and passed separately. Formatting, all-target clippy with warnings denied, dependency/panic-profile checks, strict OpenSpec validation and `git diff --check` passed. Existing Cargo future-incompatibility notices are unchanged. The draft PR's Checks panel carries the hosted results for the follow-up commit.
