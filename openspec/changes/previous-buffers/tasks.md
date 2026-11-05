# Tasks

## 1. Window history

- [x] 1.1 Add `prev`/`next` to `Window`; `show` records, `display` doesn't, `split` copies; `drop_killed` drops dead buffers from both; verify with `window.rs` unit tests
- [x] 1.2 `Windows::replace_buffer` becomes `drop_killed`: every window forgets dead buffers and an affected window shows its previous live buffer of its workspace, else the given replacement; verify with the kill scenarios in `visiting.rs` tests and the workspace-list scenario in `workspace_list.rs` tests

## 2. Commands

- [x] 2.1 Add `previous-buffer` and `next-buffer` with the fallback and errors, bound to `C-x <left>` and `C-x <right>` in `keymap.toml`; verify with harness tests for each buffer-switching scenario

## 3. Docs and checks

- [x] 3.1 Update `docs/editor.md` Buffers (per-window history, killed buffer replacement, the new keys); verify by reading the paragraph
- [x] 3.2 `cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace` pass
