# Tasks

## 1. Output selection

- [x] 1.1 Return each command's start, input, output and end as char indices from `copy_text` and keep them on the buffer in place of the prompt targets; prompt jumps read their inputs; verify the prompt-jump tests still pass
- [x] 1.2 Add `vterm-mark-output`, bound to `C-c C-r` in the `vterm-copy-mode` keymap; verify with harness tests: point on output and on the command line copy exactly the output with `RET`, the last command without `D` runs to the text's end, no marks echoes `No command at point`
- [x] 1.3 Record what was built in the Terminal paragraph of `docs/editor.md`; verify fmt, clippy and tests pass

## Workflow follow-up

- Archive the change after review.
