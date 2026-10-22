# Tasks

## 1. History and recording

- [x] 1.1 Add `back` and `forward` to `Workspace` and `Core::push_jump` (dedupe against the newest entry, a cap of 100, emptying forward). Workspace kill removes the history's markers. Verify with a unit test for the cap and dedupe.
- [x] 1.2 Split `switch_to` into `show_buffer` plus recording when the buffer changes. Record in `project_grep` for a hit in the same buffer, and in `swiper` when point moves. Verify with harness tests for a buffer switch, a grep hit in another file, a grep hit in the same buffer, swiper, a swiper preview quit with `C-g` (nothing recorded), switching to the shown buffer, and positions following edits.

## 2. Commands

- [x] 2.1 `xref-go-back` and `xref-go-forward` in `jump.rs` (skip dead and current positions, echo the end messages), registered in `builtins.rs` and bound to `M-,` and `C-M-,` in `keymap.toml`. Verify with harness tests for back after a grep jump (the acceptance criterion), back across several jumps, a killed buffer skipped, forward undoing back, a new jump clearing forward, the empty-list messages, and separate workspace histories.
- [x] 2.2 Record the behavior in `docs/editor.md` (Search and navigation) and drop "jump history" from project grep's "Not yet" list.

## 3. Checks

- [x] 3.1 `cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace` and the rest of `.github/workflows/ci.yml` pass.

## Workflow follow-up

- Archive the change after review.
