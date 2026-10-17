# Tasks

## 1. Core: open, editing keys and save

- [x] 1.1 Add `Left`, `Right`, `Up`, `Down` to `KeyCode` and the names `<left>`, `<right>`, `<up>`, `<down>` to `parse_keys`, extending its test. Verify with `cargo test -p aiditor-core key`.
- [x] 1.2 Add `Core::open(path, text)`, `Effect::Save { buffer, path, text }`, `Msg::Saved(BufferId)`, the `C-x` prefix field, and `DEL`, arrow and `C-x C-s` handling in `Ctx::key`. Make the harness run `Save` effects like the app. Add harness tests for every scenario in both delta specs that the core can check (typing, DEL, arrows, save, failed save, no file, `C-x` then another key). Verify with `cargo test -p aiditor-core`.

## 2. App: file argument, keys, drawing

- [x] 2.1 In `main.rs`, read the file argument (missing → empty, other errors exit with a message), call `Core::open`, carry out `Effect::Save` (write, then `Msg::Saved` or log), map key-down events to `Msg::Key`, draw lines with a cursor and keep point visible, set the window title. Verify with `cargo build -p aiditor` and by running `cargo run -p aiditor -- <file>`.

## 3. Integration

- [x] 3.1 Run CI's checks (`cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, the no-GPUI-or-Tokio check) and verify they pass.
- [x] 3.2 By hand on macOS and Linux: open a real file, type, use arrows and DEL, save with `C-x C-s`, and check the file on disk.

## Workflow follow-up

- Open a PR that closes #10, then archive the change.
