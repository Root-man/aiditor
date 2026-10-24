# Proposal

## Why

A terminal buffer shows only the live screen and sends nearly every key to the shell, so output that scrolled away can't be read, searched or copied. Copy mode, as vterm's `vterm-copy-mode`, turns the scrollback into a read-only buffer where the editor's motion, search and kill commands work (#234).

## What Changes

- `C-c C-t` in a terminal toggles copy mode (`vterm-copy-mode`, vterm's key under the `C-c` escape prefix kept by #89).
- Entering builds the buffer's text from the scrollback and the screen, soft-wrapped rows joined into one line, with point at the terminal's cursor. The window draws that text like any read-only buffer; keys run commands instead of reaching the shell.
- Output keeps feeding the grid but doesn't change the text or point while copy mode lasts.
- `RET` (`vterm-copy-mode-done`) copies the region, or the current line without one, to the kill ring and clipboard and leaves. Leaving empties the text and shows the live screen again.
- On the alternate screen there is no scrollback, so the text is the screen.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `terminal-buffers`: adds copy mode; a terminal's text is empty and its keys go to the shell only outside copy mode, and its window shows the live screen only outside copy mode.

## Impact

- `crates/aiditor-core`: `terminal.rs` (building the text, the commands, char mode off in copy mode), `buffer.rs` (a terminal's copy text), `frame.rs` (draw the text in copy mode), `builtins.rs`, `keymap.toml`.
- `docs/editor.md` Terminal section records the decisions.
