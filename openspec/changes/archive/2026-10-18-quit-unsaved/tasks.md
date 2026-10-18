# Tasks

## 1. Core

- [x] 1.1 Make minibuffer prompts and history keys owned `String`s; verify existing minibuffer tests pass
- [x] 1.2 Add `save-buffers-kill-terminal` bound to `C-x C-c` and `s-q`, `Core::quitting`, `Effect::Quit`, `Msg::Quit`, and the `Msg::Saved` handling; verify with harness tests for every `quitting` scenario but the window one
- [x] 1.3 Record the decisions in `docs/editor.md`

## 2. App

- [x] 2.1 Run `Effect::Quit` as `cx.quit()` and route window close to `Msg::Quit`; verify with a GPUI test that `simulate_close` keeps a window with a modified buffer open and shows the prompt

## 3. Checks

- [x] 3.1 `cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D warnings` and `cargo test --workspace` pass
