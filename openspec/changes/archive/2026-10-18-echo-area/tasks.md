# Tasks

## 1. Echo area in the frame display

- [x] 1.1 Add `message: Option<String>` to `FrameDisplay` and fill it in `Core::frame()`; update the frame split snapshots and verify `cargo test -p aiditor-core` passes
- [x] 1.2 Add a harness test that a failing command's error is in a frame snapshot and that the next key leaves the frame without a message; verify it passes
- [x] 1.3 Add harness tests that a click clears a message, that a failing key replaces one, and that scrolling keeps one; verify they pass

## 2. Checks

- [x] 2.1 Run `cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D warnings` and `cargo test --workspace` and verify all pass
