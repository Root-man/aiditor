# Tasks

## 1. Diff and apply

- [x] 1.1 Add `similar` to `aiditor-core` and write `buffer::diff` plus the pure `buffer::revert` (read result to `Revert` or the spec's error message). Verify with unit tests: the diff applied to the old text gives the new text (including no trailing newline and CRLF input), and the missing and binary errors.
- [x] 1.2 Add `Buffer::replace(&edits)` (back to front, past read-only, recorded). Verify with a unit test that a marker on an unchanged line keeps its line and column.

## 2. Plumbing and command

- [x] 2.1 Add `FileOp::Revert` and `Msg::Reverted` in `lib.rs` (`Revert` sits beside `buffer::revert`). Handle the reply in the core (drop it when stale; otherwise one undo group, set `disk`, `crlf` and `bom`, mark saved; echo errors). Answer the op in `files::run` and in the harness. Verify with `cargo build --workspace`.
- [x] 2.2 Add the `revert-buffer` command in `visiting.rs`, registered in `builtins.rs`. Verify with harness tests covering every `file-reverting` scenario: point on line `f` (the acceptance criterion), mark and a second window, a changed line, undo, saving after a revert, CRLF, a stale reply, no prompt when unmodified, the prompt's `No` and `Yes`, no file, a deleted file, and a binary file.

## 3. Docs and checks

- [x] 3.1 Record the decision in `docs/editor.md`: settle the Open questions entry and replace the "still to be decided" line under File watching with a "Built in #79" note.
- [x] 3.2 `cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace` and the rest of `.github/workflows/ci.yml` pass.
