# Tasks

## 1. Terminal directory

- [x] 1.1 Seed the terminal's directory with the spawn directory in `vterm`, and update the `Terminal` doc/spec comment; verify with a core test that a fresh `vterm`'s terminal reports its spawn directory
- [x] 1.2 Add a `Core` helper for the current buffer's file or terminal directory and use it in `Core::directory()` and `vterm`; verify with core tests that after OSC 7 `/tmp` `C-x C-f` starts at `/tmp/` and `vterm` spawns in `/tmp`, and without OSC 7 both use the start directory
- [x] 1.3 Update the terminal and find-file paragraphs of `docs/editor.md`; verify the text matches the delta specs

## 2. Checks

- [x] 2.1 `cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D warnings` and `cargo test --workspace` pass

## Workflow follow-up

- Archive the change after review.
