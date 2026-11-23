# Tasks

Each finding keeps its scan row number. #1 lands its tests; the others change no behavior, and the existing tests must pass unchanged.

## 1. One git update at a time (#1, D1)

- [x] 1.1 Write the spec's five "One git update at a time" scenarios as core tests in `diff.rs`, using the view harness (`view()`, `answer()`, `inline()`): a second `s` before git answers sends no second `GitUpdate`; `C-c s` after a successful update but before the status answer echoes `Git is still updating`; the refreshed answer, a failed update and a plain `g` each leave `s` sending a step. Verify the first two fail before the fix.
- [x] 1.2 Add `updating: bool` to `DiffList`. Set it at the two `GitUpdate` pushes in `diff_update` and `diff_update_change`, refuse at the top of `diff_update` after the revisions check, and clear it in `on_git_updated`'s error arm and in `took_answer`. Verify the 1.1 tests and every existing diff-view test pass.

## 2. Shared list writing (#2, #5, D2, D5)

- [x] 2.1 Add `Windows::iter()` yielding `(WindowId, &Window)` next to `windows_mut()` in `workspace.rs`. Use it in `frame.rs` `display_buffer`, `diff.rs` `shown_window` and `diff_update`, and `workspace_list.rs`. Verify `cargo test -p aiditor-core` passes.
- [x] 2.2 Add `Core::list_point`, `Buffer::set_lines` and `Core::place_list_point`. Rewrite `render_sessions` and `render_diff_list` on them, `top` set to `first` for sessions and `false` for the diff list. Verify the sessions-list and diff-view tests pass unchanged and both functions lose their point lookup, text and property loop, and window loop.

## 3. Display (#3, D3)

- [x] 3.1 In `display::build`, add the `fold_at` closure and use it from the main loop's fold lookup (AlignRight after it) and from `rest`. Verify the display, session, fold and align-right tests pass unchanged; if one changes because of the fold-over-align order, restore the old order and note it for the scan record.

## 4. Services git module (#4, #8, D4, D7)

- [ ] 4.1 Move the git functions and their direct tests from `files.rs` to a new `git.rs` (`pub mod git` in `aiditor-services/src/lib.rs`), with `files::run` calling `git::…`. Make `read`, `create_temp` and `NEXT_TEMP` `pub(crate)`, and point `benches/services.rs` and `aiditor/src/main.rs` at `git::snapshot` and `git::clear_snapshots`. One commit, moves only. Verify `cargo test -p aiditor-services` passes with the same test count and `git show --color-moved` shows the functions as moved.
- [ ] 4.2 Replace `check` with `git_output` and use it in `git_status`, `git_refs`, `git_diff_files`, `snapshot` (dropping its `run` closure) and `git_update`'s plain steps. Verify the git status, staging, refs, diff-files and snapshot tests pass, error texts included.

## 5. Core visibility and leftovers (#6, #9, D6)

- [ ] 5.1 Re-run the scan's grep for core `pub fn`s in public modules that `crates/aiditor`, `crates/aiditor-services` and `crates/aiditor-core/benches` never name, and make the 37 functions `pub(crate)`. Run `cargo clippy --workspace --all-targets -- -D warnings`. Then, per D6, make dead test-only helpers `#[cfg(test)]`, delete other dead functions with the tests of only them, and put anything another crate needs back to `pub`. Verify clippy is clean and `cargo test --workspace` passes.
- [ ] 5.2 Delete the stale `/// Each sessions buffer's state…` line above `Core::list_requests` in `lib.rs`. Verify `cargo doc -p aiditor-core --no-deps` shows `list_requests` with only its own comment.

## 6. Frontend (#7, #10, D8)

- [ ] 6.1 Delete the `row` closure in `Editor::render` so `line` uses the free `row`. Verify the frontend tests pass (`cargo test -p aiditor`).
- [ ] 6.2 Add `cursor_run` and use it for the text cursor and the terminal cursor in `prepaint`. Then move the terminal block into `terminal_paint` returning `TerminalPaint`, or leave it inline if it needs more than seven parameters. Verify the `terminal_cells_sit_on_the_grid`, cursor and session paint tests pass, and run the app on macOS with a terminal and a file buffer to check the block, beam and hollow cursors draw as before.

## 7. Integration

- [ ] 7.1 Run `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings` and `cargo test --workspace` on macOS; record this scan and each row's outcome in `docs/refactoring-opportunities.md`, as the earlier scans are recorded, and verify the record names all 10 rows.
