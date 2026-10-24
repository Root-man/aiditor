# Proposal

## Why

Terminals now record OSC 133 prompt marks (#243), but nothing reads them. vterm's `C-c C-p` and `C-c C-n` jump between commands in the scrollback, the first use M2 promises for the marks (#245).

## What Changes

- Each prompt mark also keeps the cursor's column, so a jump can land where the input starts rather than at the start of its line.
- Entering copy mode maps every mark to its place in copy mode's text, counting soft-wrapped rows joined into one line, and keeps one target per command: its `B` mark, else its `A`.
- `C-c C-p` (`vterm-previous-prompt`) and `C-c C-n` (`vterm-next-prompt`) in copy mode move point to the previous or next target, honouring the prefix count (negative goes the other way; a count past the last target stops at the farthest one).
- With no target in that direction, they echo `No prompt found` and leave point.

## Capabilities

### New Capabilities

### Modified Capabilities
- `terminal-buffers`: marks keep their column; copy mode gains prompt jumps.

## Impact

- `crates/aiditor-core/src/terminal.rs`: marks gain a column; `copy_text` also returns the prompt targets; two new commands.
- `crates/aiditor-core/src/buffer.rs`: a terminal buffer keeps copy mode's prompt targets.
- `crates/aiditor-core/src/builtins.rs`, `crates/aiditor-core/keymap.toml`: the commands and their keys in the `vterm-copy-mode` keymap.
- `docs/editor.md`: the Terminal paragraph records what was built.
