# Tasks

- [x] 1 Pad the frame 8 px left and right and 2 px at the bottom.
- [x] 2 Draw a block cursor with the char under it in the background color, in windows and the minibuffer; the window cursor test checks its width.
- [x] 3 Show key hint entries as `key : command`.
- [x] 4 Add `Minibuffer::count`, drawn before the prompt and left out of the line's columns; the completion list test checks `13/15 `.
- [x] 5 Keep the tick on `Buffer::append`; `the_mark_stays_active_in_messages` fails without it.
- [x] 6 Auto-save only file buffers; delete a recovered no-file recovery file; move the auto-save tests to file buffers.
- [x] 7 File the M5 `find-file` task.
- [x] 8 `cargo fmt --all --check`, `cargo clippy --all-targets -- -D warnings` and `cargo test` pass locally and in CI.
