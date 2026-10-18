# Tasks

## 1. Buffers

- [x] 1.1 Keep the recency list in `Buffers` (add last, kill drops, `raise` to front) and make names unique with `<n>`; verify with unit tests in `buffer.rs`
- [x] 1.2 Give each buffer a point marker at 0; `Window::release` records the window's point into it and `Window::new` starts there; verify existing window tests still pass

## 2. Commands

- [x] 2.1 Add `switch-to-buffer` and `switch-to-buffer-other-window` with recency completion, bound to `C-x b` and `C-x 4 b`, and route `Core::open` through the same switch; verify with harness tests for every `buffer-switching` switching scenario, point restore included
- [x] 2.2 Add `kill-buffer` bound to `C-x k`, with the modified-file confirmation and window replacement; verify with harness tests for every kill scenario
- [x] 2.3 Record the decisions in `docs/editor.md` under Commands and keymaps; verify the text names the defaults chosen here

## 3. Checks

- [x] 3.1 `cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D warnings` and `cargo test --workspace` pass

## Workflow follow-up

- Archive the change before pushing the PR.
