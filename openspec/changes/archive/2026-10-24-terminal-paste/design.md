# Design

## Context

`s-v` is a super key, so char mode leaves it to the keymaps, where global binds it to `yank`. Terminal buffers have major mode `vterm-mode`, whose keymap is looked up before global. A command whose spec sets `clipboard` gets the clipboard's text before its key (`Core::wants_clipboard`).

## Decisions

- **A `vterm-yank` command bound in `[vterm-mode]`**, with `clipboard: true`, so the frontend sends the clipboard before `s-v` with no new plumbing. It takes in foreign clipboard text the same way `yank` does, then writes the newest kill as one `TerminalOp::Write`. Alternative: special-casing `s-v` in char mode — rejected, it bypasses rebinding and the clipboard read.
- **`Terminal::paste(text) -> Vec<u8>`** reads `TermMode::BRACKETED_PASTE` from the term, as `encode` reads DECCKM. Bracketed: `\x1b[200~` + text without ESC + `\x1b[201~`. Otherwise the bytes as is.

- **Clipboard keys per platform.** `keymap::BUNDLED` is the shared `keymap.toml` plus `keymap-macos.toml` or `keymap-linux.toml`, chosen with `cfg!`, bound in order before the user file. macOS binds `s-c`/`s-x`/`s-v`, Linux `C-S-c`/`C-S-x`/`C-S-v`, each with `vterm-yank` on its paste key. On Linux `C-S-v` joins `editor_key`'s list, since char mode would otherwise send it as `^V`; `C-v` still reaches the shell, and on macOS so does `C-S-v`. Alternative: the frontend translating `C-S-` to `s-` on Linux — rejected, hints and the user keymap would name the wrong keys.

## Risks / Trade-offs

- [Without bracketed paste, a multi-line paste runs line by line] → that is what the issue asks; programs that care turn on 2004.
