# Tasks

## 1. Auto-revert

- [x] 1.1 Make `Buffers::iter_mut` yield `(BufferId, &mut Buffer)`, move `on_checked` to `Ctx` and push `FileOp::Revert` for an unmodified buffer whose disk hash differs from the checked one; update the `Msg::Checked` dispatch. Verify with `cargo build --workspace`.
- [x] 1.2 Harness tests for every scenario of the new `file-watching` requirement: the save in another workspace (the acceptance criterion; the saving buffer gets no revert), another program's write (point, undo, no echo), a modified buffer, and a deleted file coming back. Update the #80 test that expected a refused save after the file came back.

## 2. Docs and checks

- [x] 2.1 Add a "Built in #81" note under File watching in `docs/editor.md` and update #80's "Not yet" line.
- [x] 2.2 `cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace` and the rest of `.github/workflows/ci.yml` pass.
