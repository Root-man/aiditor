# Proposal

## Why

Char mode ([#232](https://github.com/Root-man/aiditor/issues/232)) sends typed keys to the shell, but `s-v` still runs `yank`, which fails in the read-only terminal buffer. Issue [#233](https://github.com/Root-man/aiditor/issues/233) makes paste reach the shell, safely for programs that asked for bracketed paste.

## What Changes

- A `vterm-yank` command writes the newest kill to the current terminal's shell, first taking in clipboard text from another app as `yank` does. The `vterm-mode` keymap binds `s-v` and `C-S-v` to it; `C-S-v` stays with the editor in char mode, as in Linux terminals, since KDE and GNOME take Super keys.
- When the program turned on bracketed paste (mode 2004), the text is wrapped in `\x1b[200~` and `\x1b[201~` with every ESC removed, as Zed does; otherwise it is sent as is.
- Defaults picked: `C-y` keeps reaching the shell (readline's yank), as #89 decided; no prefix argument or `M-y` in terminals.
- Not in scope: copy mode (#88), image paste, rewriting newlines to `\r` without bracketed paste.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `terminal-buffers`: `s-v` pastes into the shell, bracketed when the program asks.

## Impact

- `aiditor-core`: `terminal.rs` (`Terminal::paste`, `vterm-yank`), `kill_ring.rs` (a newest-kill helper for it), `builtins.rs`, `keymap.toml`.
- `docs/editor.md`: the terminal paragraph records paste as built.
- No new dependencies.
