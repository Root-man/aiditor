# Tasks

## 1. Property test

- [x] 1.1 Add a proptest in `crates/aiditor-core/src/buffer.rs` tests: random inserts/deletes with random undo boundaries on a buffer with Stay and Advance markers; assert markers stay in `0..=len_chars` after every edit, undo and redo, undo-all yields the original text and redo-all the final text. Verify with `cargo test -p aiditor-core`.
- [x] 1.2 Fix any bug the property exposes in `buffer.rs`, keeping the shrunk case in `proptest-regressions/`. Verify with `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings` and `cargo fmt --all --check`.

## Workflow follow-up

- Archive the change after verify passes.
