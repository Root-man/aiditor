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

## Stop point requested by the user

Stages 1–3 are complete (11/53 tasks). Stop after window-configuration consolidation; stage 4 and later stages are not started. The change is not ready to archive.

- Final current-tree macOS checks: `cargo fmt --all --check`, `cargo clippy --all-targets -- -D warnings`, and `cargo test --quiet` passed. Tests: 21 frontend + 356 core + 45 services + 1 boundary + 1 crash recovery + 4 file sync = 428 passed, zero failed. Native watcher/file-sync tests ran outside the filesystem sandbox.
- Structural check: Core owns one `Windows`; inactive workspaces store `Option<Windows>`; switching swaps the whole value; active and saved configurations use the same killed-buffer replacement helper.
- Behavioral coverage: existing window/display/workspace, marker release, preview cancellation and cross-workspace buffer tests pass; the new switching test preserves selected window, splits, point, mark, mark ring and scroll, and drops an inactive window click. No snapshot updates were required.
- Stage 4 was not implemented: `FileOp::Open` is still path-based and the deferred grep jump remains unchanged.
- Linux and manual GUI flows were not run. They remain part of the unfinished whole-change verification, not claims of completion at this stop point.
