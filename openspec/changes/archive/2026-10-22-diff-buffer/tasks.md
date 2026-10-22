# Tasks

## 1. Diff text

- [x] 1.1 Factor revert's read/decode into a shared helper and add `buffer::unified_diff` in `buffer.rs`. Verify with a unit test: header, context of three lines, empty for no differences, and the missing/binary errors (revert's tests still pass).
- [x] 1.2 Add `Buffer::set_text` (past read-only, no undo, `modified` kept). Verify with a unit test.

## 2. Plumbing and command

- [x] 2.1 Add `FileOp::Diff` and `Msg::Diffed` in `lib.rs`; answer the op in `files::run` and the harness. Verify with a `files.rs` test and `cargo build --workspace`.
- [x] 2.2 Add `diff-buffer-with-file` and `on_diffed` in `visiting.rs` (fill `*Diff*`, display it in another window, messages, stale drop), register it in `builtins.rs`. Verify with harness tests for every `file-diffing` scenario, the recovered-buffer one (the acceptance criterion) included.

## 3. Docs and checks

- [x] 3.1 Update `docs/editor.md`: recovery's "Not yet: compare with the disk" and a "Built in #82" note after Reverting.
- [x] 3.2 `cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace` and the rest of `.github/workflows/ci.yml` pass.
