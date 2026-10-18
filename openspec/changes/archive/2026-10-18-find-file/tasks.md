# Tasks

## 1. File service

- [x] 1.1 Add `files::list` (sorted names, `/` after directories, symlinks followed); verify with a unit test in `files.rs`

## 2. Core

- [x] 2.1 Add `ArgSpec::files`, `Completion::dir`, `Effect::ListDir`, `Msg::Listed`; start file prompts at the buffer's directory and list the typed directory from `poll_completion`; verify with harness tests for every completion scenario in the `file-visiting` delta
- [x] 2.2 Full-path `TAB`/`RET`, `RET` entering a directory, and `minibuffer-exit-input` on `M-RET`; verify with harness tests for the TAB/RET scenarios and the minibuffer delta
- [x] 2.3 Add `find-file` on `C-x C-f`, `Effect::Open`, `Msg::Opened` with switching to a visited buffer and refusing binary files; verify with harness tests for the visit, switch and refuse scenarios
- [x] 2.4 Carry out `ListDir` and `Open` in the harness as the app does

- [x] 2.5 `DEL` deletes a whole directory in file prompts (`minibuffer-directory-delete-char`), checking the loaded listing; verify with a harness test for each scenario plus prefix, region, point and other prompts

## 3. App and docs

- [x] 3.1 Run `ListDir` and `Open` on Tokio's blocking pool in `main.rs`
- [x] 3.2 Record the decisions in `docs/editor.md` under Commands and keymaps

## 4. Checks

- [x] 4.1 `cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D warnings` and `cargo test --workspace` pass
