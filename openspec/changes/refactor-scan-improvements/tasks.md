# Tasks

## 1. Reading and decoding files (rows 1, 3)

- [x] 1.1 In `crates/aiditor-services/src/files.rs`, make `read` refuse anything that is neither a regular file nor a directory, before opening it. Verify with tests that a FIFO returns an error without blocking, that `read` of a directory keeps its OS error, and that `find-file`'s directory test still passes.
- [x] 1.2 Give `FileOp::Preview` the same check plus `PREVIEW_LIMIT` of 10 MiB, answering `Err` for non-regular files and `Ok(None)` over the limit. Verify with service tests for a FIFO, a sparse file over the limit and a file at the limit, and a core test that a later selection previews after a refused one.
- [x] 1.3 In `buffer::decode`, set `crlf` only when every `\n` follows `\r`. Verify with `decode` and `encode` cases for `a\r\nb\n` and `a\r\nb`, the visiting test for a save that changes only the edit, and the existing `saves_keep_the_bom_and_crlf`.
- [x] 1.4 Update `docs/editor.md` for non-regular files, the preview limit and mixed line endings, and verify with `cargo test -p aiditor-core -p aiditor-services`.

## 2. Saves (rows 6, 9)

- [x] 2.1 Replace both `fs::write` calls in `files::write` with one `write_synced` helper. Verify with the existing new-file, dangling-symlink and hard-link tests, which must still pass, and by reading that no plain `fs::write` remains there.
- [x] 2.2 Open the canonical target for writing before choosing how to write, and return only `PermissionDenied`. Verify with a test that a 0444 file in a writable directory keeps its content and errors, skipped under root, and that `writes_in_place_when_the_directory_is_read_only` still passes.
- [x] 2.3 Update `docs/editor.md` for durability and refused writes, and verify with `cargo test -p aiditor-services`.

## 3. Repository filters (row 11)

- [x] 3.1 In `files::changed`, run `git config --show-scope --get-regexp '^filter\.'` first and apply the scope and exit-code rules from `design.md`. Verify with a test whose repository-local filter would create a marker file: the marker never appears and the file counts as unchanged.
- [x] 3.2 Verify a globally configured filter keeps status working, using a test with `GIT_CONFIG_GLOBAL` pointing at a temp file. Note the change in `docs/editor.md`.

## 4. Agent stderr (row 2)

- [x] 4.1 Add a raw-bytes stderr step to `aiditor-fake-agent` and document it in `docs/editor.md`'s fake agent script section. Verify with `crates/aiditor-services/tests/fake_agent.rs`.
- [x] 4.2 Read stderr with `read_until` and `from_utf8_lossy` in `crates/aiditor-services/src/agents.rs`. Verify with `crates/aiditor-services/tests/agents.rs` tests that a bad byte gives U+FFFD and later lines, that more than a pipe's worth of stderr after it still reaches the exit message, and that `stderr_then_exit` still passes.

## 5. Session code cleanup (rows 8, 7, 16)

- [x] 5.1 Move the render functions from `agents.rs` to `agents/render.rs`, changing only visibility. Verify `cargo test -p aiditor-core agents` passes with no test logic changed.
- [x] 5.2 Delete `Session.updates` and its `shortcut:`, and turn `agent_update` into one `match update.update` by value. Verify the session tests pass with the `updates.len()` assertion replaced by a check that an update for an ended session changes no text, plus a test for an unknown agent id.
- [x] 5.3 Add `options(select)` and use it in `values` and `mode_line`. Verify the mode-line tests still pass.

## 6. Command output rendering (row 5)

- [x] 6.1 Add `hot-stream/tool-output-*` cases to `crates/aiditor-core/benches/hot_paths.rs` for shown-folded, shown-open and hidden calls with about 5 MB of output. Verify they run and record the baseline numbers.
- [x] 6.2 Keep `Processor` and a named `Sgr` on `Term`, feed chunks once in `terminal_meta`, and have `tool_content` read the kept text and faces. Verify the existing command-output tests pass, plus a new test with an escape split across two chunks.
- [x] 6.3 Check `shown` for dirty tools in `render_streams`. Verify a test that a hidden call's block stays unchanged until the first frame showing it, for both a hidden buffer and an inactive workspace.
- [x] 6.4 Add the folded-call short path, and a `shortcut:` comment naming the open-call limit. Verify a test that folded and open calls hold the same text and faces over the same chunks, covering a split escape, a trailing newline and a backtick run that forces the full path.
- [x] 6.5 Make folded display range expansion jump across complete hidden ranges in `display.rs`, preserving chained folds and downstream faces. Verify a large-fold display regression and the existing fold tests.
- [x] 6.6 Update `docs/editor.md` and record the before and after numbers from 6.1, and verify the shown-folded and hidden cases no longer grow with total output.

## 7. Project grep (rows 4, 19)

- [x] 7.1 Cut listed text to `columns` in `minibuffer()`. Verify core tests for a 1 MB line, a short line and a multibyte line, and that `RET` reaches a match past the cut.
- [x] 7.2 Keep the file-to-group map and running line-number width in `ProjectGrep`, and read hits by index in `minibuffer()` and `grep::preview` through `Prompts::selected_index`. Add a `shortcut:` comment for the labels `set_items` still clones. Verify the grep ordering, grouping and preview tests pass with their expectations unchanged.
- [x] 7.3 Update `docs/editor.md` for the cut rows, and verify with `cargo test -p aiditor-core grep`.

## 8. Workspace kill (row 10)

- [x] 8.1 Move the `PROCESSES` question into one `Ctx` helper, keeping `save_buffers_kill_terminal`'s behavior. Verify the existing quit tests in `terminal.rs`, `agents.rs` and `visiting.rs` pass unchanged.
- [x] 8.2 Ask it in `Ctx::kill_workspace` for that workspace's live terminals and unended sessions. Verify tests for each `workspaces` scenario, and update any test that kills a live workspace through keys to answer the question.
- [x] 8.3 Update the workspace section of `docs/editor.md`, and verify with `cargo test -p aiditor-core workspace`.

## 9. Core structure (rows 12, 13, 17, 18)

- [x] 9.1 Group the auto-save fields and `recovered` into `autosave::Autosave`, and the watch fields into `watching::Watch`. Verify `cargo test -p aiditor-core` passes with only field paths changed in tests.
- [x] 9.2 Add `unselected`, `project_candidates` and `grep_hits` to `ArgSpec`, set them in `builtins.rs`, and remove the command-name checks from `minibuffer.rs` non-test code. Verify by grep that no such string remains there, and that the prompt tests pass.
- [x] 9.3 Add `project::is_ignore_file`, use it in `watcher.rs` and `file_index.rs`, and pass `&Path` to `configure_walker`. Verify with `cargo test -p aiditor-services` and the file-index tests.

## 10. Frontend (rows 14, 15)

- [x] 10.1 Split `Editor::render` into `echo_area`, `hint_rows` and `workspace_bar`, keeping every `debug_selector`. Verify with `cargo test -p aiditor`, which runs on GPUI's test platform and opens no real window.
- [x] 10.2 Keep the last title in `Editor` and skip the platform call when it matches. Verify `the_window_title_follows_the_current_workspace` passes.

## 11. Integration

- [ ] 11.1 Verify `cargo test --workspace`, `cargo clippy --workspace --all-targets` and `cargo fmt --check` all pass locally on macOS. Linux is verified through the draft-PR CI in 11.3.
- [x] 11.2 Verify `openspec validate refactor-scan-improvements --strict` passes, and that `crates/aiditor-core` still has no gpui or tokio dependency.
- [ ] 11.3 Open a draft pull request and verify Linux and macOS CI pass, since this work is done on macOS.

## Workflow follow-up

- Archive the change after review and sync its delta specs.
