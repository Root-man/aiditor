# Tasks

Each finding keeps its scan row number. Every task lands its own test.

## 1. Session buffer freezes and panics (#1, #2, #3)

- [x] 1.1 #1: hoist `text.chars().count()` out of the filter in `agents/render.rs` `user_lines`. Verify with a test that a prompt holding a folded 20,000-line paste is sent and its echo gets one `session-user` line property per line, and that the existing `user_lines` tests pass.
- [x] 1.2 #2 (D1): clamp `top_row` to `row_starts(..).len() - 1` in `frame.rs` `window_rows` and in `Window::shown`. Verify with a test: a wrapped transcript line at the top with `top_row > 0`, point moved to the prompt, the window widened; the frame builds without a panic and shows the line.
- [x] 1.3 #3 (D2): in `paint_prompt`, call `remove_properties` only when `properties_near(prompt range)` holds a property inside it. Add a `hot_paths` case typing one char in the prompt of the 10k-property session; verify chip and paste tests pass and the new case's median drops against its baseline taken before the change.

## 2. Session commands (#5, #20)

- [x] 2.1 #5 (D3): `agent_reject_once` takes the sendable ask at point first, else the oldest. Verify with the spec scenarios "Rejecting the request at point" (session-buffers) and "Skipping the question at point" (agent-questions), and that the existing `C-c C-k` tests pass. Update the command's doc comment.
- [x] 2.2 #20: in `take_place`, when the source session is pinned, rewrite its pin to the fork's session id and emit `WritePins`. Verify with the "A pinned session stays pinned" scenario as a core test that checks the written pins.

## 3. Files and saving (#6, #7, #9, #12, #19)

- [x] 3.1 #6: `revert-buffer`'s choices become `["No", "Yes"]`. Verify with the "RET keeps the edits" scenario.
- [x] 3.2 #7 (D4): add `VISIT_LIMIT = 256 MiB` in `aiditor-services/src/files.rs`; `FileOp::Open` checks the size before reading and fails with `<path>: file too large (<n> MiB)`. Verify with a services test on a sparse 300 MiB file (`set_len`, as the preview test does) that nothing is read and the error is returned, and record the limit in `docs/editor.md` beside the 10 MiB preview note.
- [x] 3.3 #12 (D5): in the `Update` arm of the write, return `Ok(hash)` when the disk already holds `bytes`. Verify with a services test saving the same bytes twice with the old expectation (both succeed) and a different-bytes case that still conflicts.
- [x] 3.4 #19: `edits.rs` `prepare` sets point on every window of the terminal's workspace showing the buffer when the request has a position. Verify with a test: the selected window already shows the file at line 1, a request with `+3` arrives, and point is on line 3.
- [x] 3.5 #9 (D10): `project_file_prompt` opens with `Some(&[])` and passes the project's files to `add_candidates`. Verify with the existing test that a second `C-x p f` lists the files.

## 4. Grep (#4, #8, #15, #18)

- [x] 4.1 #18: compute the hit column with `text.get(..start).map_or(0, |s| s.chars().count())` in `aiditor-services/src/grep.rs`. Verify with a unit test on a line holding `é` and a match start inside it.
- [x] 4.2 #8: `ProjectGrep::label` drops `:{hit.text}`. Update the assert at `grep.rs:778`; verify the project-grep tests pass and two hits on one line can't occur (one `Hit` per line).
- [x] 4.3 #4 (D6): move `said`, `SAID_CHARS` and their tests from `aiditor-core/src/grep.rs` to the services grep; apply them in the worker when the search has session `names`; the core uses `Hit.text` as given. Verify the session-search tests in both crates pass and a hit's `text` from a long JSONL line arrives at most `SAID_CHARS` long.
- [x] 4.4 #15: the core puts the line number's range with `GrepLineNumber` into `mb.syntax[i]` for numbered rows; delete `Minibuffer::numbered` and the frontend's `find(':').expect(..)` arm in `element.rs`. Verify with a core test on the faces of a numbered grep row and the existing frontend grep render test.

## 5. Display and syntax (#10, #14, #16)

- [x] 5.1 #10 (D7): one `syntax::language(name)` table used by `for_file` and the Markdown fence lookup; delete the second match. Verify with the "Full names and other extensions" scenario and the existing extension tests.
- [x] 5.2 #16: guard the `Cache::edit` calls in `syntax.rs` with `grammar.locals.is_some()`. Verify with a test that editing a `.md` buffer leaves the locals cache's edited set empty and Markdown highlighting tests pass.
- [x] 5.3 #14 (D8): `display::build` counts visual rows as lines finish and stops at `rows`. Add a `hot_paths` case with 600-char paragraphs in a wrapped 50-row window; verify the wrap and session display tests pass unchanged and the new case lays out about `rows` rows (assert the built line count).

## 6. Frontend and logging (#11, #17)

- [x] 6.1 #17: `Core::log` removes the oldest lines past 1,000 without undo. Verify with the "The oldest line goes" scenario and that `*Messages*` tests pass. Record the cap in `docs/editor.md`.
- [ ] 6.2 #11: after `Msg::Resize` in `element.rs` `prepaint`, call `window.request_animation_frame()`. Check on macOS in a running window that maximising re-lays out a truncated workspace modeline at once; if it already did before the change, drop it and note why in the scan record.

## 7. Split `agents.rs` (#13, D9)

- [x] 7.1 Move `agents.rs`'s `mod tests` to `agents/tests.rs` (`#[cfg(test)] mod tests;`). Verify `cargo test -p aiditor-core` passes with the same test count.
- [x] 7.2 Move asks and their answer commands to `agents/asks.rs`, tool and terminal calls to `agents/tools.rs`, config options and the mode line to `agents/config.rs`, one commit each with no edits besides `use` lines and visibility. Verify each commit builds and `cargo test -p aiditor-core` passes.

## 8. Integration

- [x] 8.1 Run `cargo test --workspace`, `cargo clippy --workspace` and the `hot_paths` benches; record the run and this scan in `docs/refactoring-opportunities.md` and verify the record names each row's outcome.
