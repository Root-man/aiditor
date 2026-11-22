# Tasks

## 1. File service

- [x] 1.1 Add `GitStep`, `FileOp::GitUpdate` and `Msg::GitUpdated` in `aiditor-core/src/lib.rs`, and the harness's no-answer arm; `cargo check --workspace` passes
- [x] 1.2 Run the steps in `aiditor-services/src/files.rs` (filter refusal, stop at the first failure, git's stderr as the error); tests against real repositories cover every `git-staging` scenario

## 2. The list's actions

- [x] 2.1 Refresh re-asks the shown diff and keeps point near a row that left; tests for both
- [x] 2.2 `diff-stage`, `diff-unstage`, `diff-discard` with targets from point, the unsaved-edits refusals, the discard question, `Nothing to …`, and the answer refreshing or echoing; bound to `s`, `u`, `k` in `diff-list-mode` with hints; harness tests cover every new `diff-view` scenario
- [x] 2.3 Record "Staging as built (#510)" in `docs/editor.md` and check the change validates with `openspec validate diff-stage`
