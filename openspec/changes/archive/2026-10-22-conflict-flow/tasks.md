# Tasks

## 1. Conflicted state

- [x] 1.1 Add `conflict: Option<u64>` and `conflict_asked: bool` to `Buffer`; mark and clear them in `on_checked`, clear them in `on_reverted` and a successful `on_saved`; a failed save checks the file. Show ` [conflict]` in the mode line. Verify with harness tests in `watching.rs` (marked, mode line, file going back, deleted, missed change).

## 2. The question

- [x] 2.1 Add `Ctx::resolve_conflict` in `visiting.rs` (prompt, Keep mine, Take disk, Compare), the `resolve-conflict` command, and route `save-buffer` through it. Verify with harness tests for each answer, from save and from `M-x`.
- [x] 2.2 Track the focused buffer in `Core` and ask once per disk version on focus at the end of `Core::handle`, never over an open minibuffer. Verify with harness tests: `C-x b` asks, `C-g` then away and back doesn't, conflicted while selected doesn't.

## 3. Docs and checks

- [x] 3.1 Add a "Built in #83" note to `docs/editor.md` and update #81's note.
- [x] 3.2 `cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace` and the rest of `.github/workflows/ci.yml` pass.
