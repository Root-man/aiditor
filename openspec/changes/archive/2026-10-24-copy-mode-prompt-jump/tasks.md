# Tasks

## 1. Prompt jumps

- [x] 1.1 Keep each mark's column in `Terminal::marks`; update the mark tests and verify `cargo test -p aiditor-core terminal` passes
- [x] 1.2 Map marks to copy-mode positions in `copy_text` (B else A per command, wrapped rows joined) and keep them on the buffer while in copy mode; verify with a core test of a wrapped command line
- [x] 1.3 Add `vterm-previous-prompt` and `vterm-next-prompt`, bound to `C-c C-p` and `C-c C-n` in the `vterm-copy-mode` keymap; verify with harness tests: three commands back and forth, `C-u 2 C-c C-p`, no marks echoes `No prompt found`
- [x] 1.4 Record what was built in the Terminal paragraph of `docs/editor.md`; verify fmt, clippy and tests pass

## Workflow follow-up

- Archive the change after review.
