# Tasks

## 1. Split main.rs (commit 1: pure move)

- [ ] 1.1 Move `Editor`, `Debug`, `impl Editor` (effects, mouse handlers, `font`, `painted_at`), `spawn_heartbeat` and `impl Render for Editor` to `crates/aiditor/src/editor.rs`; verify the moved code is byte-identical apart from `use`s and `pub(crate)` (`git diff --stat -M` shows moves, `git diff --color-moved` shows no edited lines)
- [ ] 1.2 Move `to_key`, `impl EntityInputHandler for Editor` and the IME helpers `ime_point`, `to_buffer`, `buffer_range` (as a second `impl Editor` block) to `crates/aiditor/src/input.rs`, with the same check
- [ ] 1.3 Move `Painted`, `Row`, `FrameElement`, `to_bounds`, `text_run`, `echo_line`, `line_runs` to `crates/aiditor/src/element.rs`, with the same check
- [ ] 1.4 Leave `USAGE`, `Args`, `parse_args`, `LogPipe`, `messages_layer`, `read_config`, `FONT` and `main` in `main.rs` with `mod editor; mod element; mod input;`; verify `main.rs` holds only startup code
- [ ] 1.5 Move tests with their code: arg and logging tests stay in `main.rs`, `sorts_command_keys_from_typing` and `key` to `input.rs`, the gpui tests, their helpers and `the_echo_line_is_one_string_with_point_in_it` to `editor.rs`; verify `cargo test -p aiditor` runs the same test count as before
- [ ] 1.6 Verify `cargo fmt --all --check`, `cargo clippy --all-targets -- -D warnings` and `cargo test` pass, then commit the move alone

## 2. Frame display carries hint and title (commit 2)

- [ ] 2.1 Add `hint: Option<Hint>` and `title: String` to `FrameDisplay` in `crates/aiditor-core/src/display.rs` and fill them in `Core::frame` (title: current buffer's name plus ` *` when modified); verify it builds
- [ ] 2.2 Add a harness test that the frame display's title is `t.txt` for an unmodified buffer and `t.txt *` after typing, and one that `frame().hint` matches `hint()` after the delay; verify both pass
- [ ] 2.3 Make `Core::hint` and `Core::minibuffer` `pub(crate)`; verify the core crate builds and `cargo tree -p aiditor-core` still shows neither gpui nor tokio

## 3. Frontend draws from one frame display (commit 2)

- [ ] 3.1 Change `FrameElement` to `{ editor: Entity<Editor>, windows: Vec<FrameWindow> }`; in `render` call `self.core.frame()` once, draw the echo area, minibuffer, hint and `window.set_window_title(&frame.title)` from it, and pass `frame.windows` to the element; verify `render` no longer calls `core.minibuffer()`, `core.hint()`, `core.message()` or `core.current()`
- [ ] 3.2 In `prepaint`, when the size differs from `reported`, send `Msg::Resize` and replace `self.windows` with `core.frame().windows`; shape `mem::take(&mut self.windows)`; keep `debug.shape` timing shaping only; verify by review that `core.frame()` is called only in `render` and in the resize branch
- [ ] 3.3 Switch the `aiditor` tests from `e.core.minibuffer()` to `e.core.frame().minibuffer` (`closing_the_window_offers_to_save`, `a_prompt_reads_typed_text`, `the_echo_line_is_one_string_with_point_in_it`, `m_x_runs_a_command_by_name`, `a_completing_prompt_filters_and_picks`); verify all gpui tests in the crate pass
- [ ] 3.4 Verify `cargo fmt --all --check`, `cargo clippy --all-targets -- -D warnings` and `cargo test` pass on Linux, and CI passes on macOS

## 4. Manual check on macOS

- [ ] 4.1 Run the app on macOS and check by hand: live resize keeps windows laid out without a one-frame lag; typing and the cursor; IME marked text (e.g. Japanese input) composes and commits; `M-x` and `C-x b` minibuffer with the completion list and highlight; the `C-x` hint appears after 1 s above the echo area and shrinks the windows; the window title shows the buffer name and ` *` after an edit; `--debug` stats still show plausible figures

## Workflow follow-up

- Open one PR with the two commits; archive the change after review.
