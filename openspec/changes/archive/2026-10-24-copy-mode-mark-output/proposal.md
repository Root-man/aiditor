# Proposal

## Why

Terminals record OSC 133 marks (#243) and copy mode jumps between prompts (#245). The other use M2 promises for the marks is per-command output capture: selecting what one command printed so `RET` copies it (#246).

## What Changes

- Entering copy mode maps each command's `C` (output start) and `D` (end) marks into copy mode's text, beside its input start.
- `C-c C-r` (`vterm-mark-output`) in copy mode sets an active region over the output of the command at point: from its `C` mark to its `D` mark, else to the next command's prompt, else to the text's end. The newline that ends the output's last line is left out, as `RET` leaves out a line's newline. `RET` then copies it as usual.
- The command at point is the last one whose prompt starts at or before point, so point on its command line or its output both work.
- With no command at point, or one with no `C` mark, it echoes `No command at point`.

## Capabilities

### New Capabilities

### Modified Capabilities
- `terminal-buffers`: copy mode gains marking a command's output.

## Impact

- `crates/aiditor-core/src/terminal.rs`: `copy_text` returns each command's marks as char indices in place of prompt places; a new command.
- `crates/aiditor-core/src/buffer.rs`: a terminal buffer keeps copy mode's commands in place of its prompt targets.
- `crates/aiditor-core/src/builtins.rs`, `crates/aiditor-core/keymap.toml`: the command and its key in the `vterm-copy-mode` keymap.
- `docs/editor.md`: the Terminal paragraph records what was built.
