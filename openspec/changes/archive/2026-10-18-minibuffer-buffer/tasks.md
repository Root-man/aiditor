# Tasks

## 1. Core minibuffer

- [x] 1.1 Open the minibuffer from `try_call` when a prompted string is missing (fresh `minibuffer-mode` buffer, window under the reserved id, selected; fail when already active) and update `calls_a_command_by_name_with_its_args`; verify with a harness test that a key script answers a prompt, two prompts chain and the prefix argument is kept
- [x] 1.2 Add `exit-minibuffer` and `abort-minibuffers` and the `[minibuffer-mode]` keymap with `RET`, `C-g`, `M-p`, `M-n`; verify with harness tests for abort and for editing keys inside the minibuffer
- [x] 1.3 Add per-prompt history with `previous-history-element` and `next-history-element`; verify with harness tests for older entries, restoring typed text, both ends and separate prompts
- [x] 1.4 Make split, `other-window` and `delete-other-windows` fail while the minibuffer is selected and a click abort it; verify with harness tests
- [x] 1.5 Add the minibuffer to `FrameDisplay` and `Core::minibuffer()`; verify with a frame snapshot test

## 2. Frontend and docs

- [x] 2.1 Draw the minibuffer in the echo area line (prompt, text, cursor, bracketed message); verify with a GPUI test that typing at a prompt reaches the minibuffer and RET returns it
- [x] 2.2 Record the minibuffer decisions in `docs/editor.md`'s Minibuffer bullet and the commands spec delta; verify `openspec validate minibuffer-buffer` passes
- [x] 2.3 Run `cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D warnings` and `cargo test --workspace` and verify they pass

## Workflow follow-up

- Verify, commit, archive, open the PR and merge as `/implement` says.
