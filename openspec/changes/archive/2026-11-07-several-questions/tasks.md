# Tasks

## 1. Questions

- [x] 1.1 Split `Question` into the block and its `Field`s; parse several choice fields with paired companions, in natural key order; verify with `questions.rs` tests for the Claude and codex shapes
- [x] 1.2 Lines of several fields as frame 6e, the open field's rows and preview column shifted; verify with a `questions.rs` test on the lines
- [x] 1.3 Recorded answers, merged content, per-field range checks and the answered line; verify with `questions.rs` tests

## 2. Session

- [x] 2.1 Redraw a block whose length changes, moving point to the open field; `TAB` in the block opens the next field, elsewhere it still toggles; verify with core tests
- [x] 2.2 A single choice records and moves on; `C-c C-y` sends all in the block; `C-c C-k` skips all; verify with core tests

## 3. Docs and checks

- [x] 3.1 Record several questions in `docs/editor.md`
- [x] 3.2 `cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace` pass
