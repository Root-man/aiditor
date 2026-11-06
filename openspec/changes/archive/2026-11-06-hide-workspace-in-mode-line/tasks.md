# Tasks

## 1. Mode line

- [x] 1.1 Add `workspace: Option<&str>` to `frame::window_display` and strip `<{workspace}>` (with `^J` for newlines) from the end of a fileless buffer's mode line name there. Pass the current workspace from `frame.rs` and `None` from `minibuffer.rs`. Verify that `cargo build` passes on the workspace.
- [x] 1.2 Add display tests for the spec's new scenarios: `-- *scratch*` for `*scratch*<work>`, `vterm1` for `vterm1<work>`, `%% *claude stderr*` for `*claude stderr*<work>`, `*claude stderr*<work><2>` left whole, and `%% *Messages*`. Verify that `cargo test -p aiditor-core` passes.
- [x] 1.3 Update the tests and insta snapshots that expect the suffix, at least `an_empty_buffer_has_one_line` and `a_fold_joins_lines` (` *scratch*<main>` becomes ` *scratch*`). Accept them with `cargo insta review` after checking that only the suffix changed. Verify that `cargo test --workspace` passes.

## 2. Docs

- [x] 2.1 In `docs/editor.md`, where the mode line is described, note that a scoped buffer's mode line leaves out the current workspace's `<workspace>` suffix because the workspace modeline shows it. Verify that the note is present.
