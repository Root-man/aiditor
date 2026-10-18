# Tasks

## 1. Setup

- [x] 1.1 Add `nucleo` to `aiditor-core` and verify `cargo tree -p aiditor-core` shows no gpui or tokio

## 2. Core completion

- [x] 2.1 Add `ArgSpec::completion`, the core's `Nucleo` matcher, feeding it when a completing minibuffer opens, the `minibuffer-completion` minor mode, and the per-message pattern update and tick with the repoll timer; make the harness tick until matching settles; verify with harness tests that typing filters candidates, empty text lists all, and a plain prompt has no list
- [x] 2.2 Add `minibuffer-next-completion`, `minibuffer-previous-completion`, `minibuffer-complete` and the `[minibuffer-completion]` keymap, and make `exit-minibuffer` return the selected candidate; verify with harness tests for moving, clamping, reset on typing, TAB, RET with and without a match
- [x] 2.3 Add `candidates` and `selected` to `display::Minibuffer` with the 10-row window that holds the selection; verify with insta snapshots of the list and update the existing minibuffer snapshot

## 3. Frontend and docs

- [x] 3.1 Draw the candidates below the prompt line with the selected one on the region background; verify with a GPUI test that typing at a completing prompt narrows `e.core.minibuffer()`'s candidates and RET returns the selection
- [x] 3.2 Record the completion decisions in `docs/editor.md`'s Minibuffer bullet; verify `openspec validate fuzzy-completion-list` passes
- [x] 3.3 Run `cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D warnings` and `cargo test --workspace` and verify they pass

## Workflow follow-up

- Verify, commit, archive, open the PR and merge as `/implement` says.
