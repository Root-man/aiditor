# Design

## Context

`s-v` is a super key, so char mode leaves it to the keymaps, where global binds it to `yank`. Terminal buffers have major mode `vterm-mode`, whose keymap is looked up before global. A command whose spec sets `clipboard` gets the clipboard's text before its key (`Core::wants_clipboard`).

## Decisions

- **A `vterm-yank` command bound in `[vterm-mode]`**, with `clipboard: true`, so the frontend sends the clipboard before `s-v` with no new plumbing. It takes in foreign clipboard text the same way `yank` does, then writes the newest kill as one `TerminalOp::Write`. Alternative: special-casing `s-v` in char mode — rejected, it bypasses rebinding and the clipboard read.
- **`Terminal::paste(text) -> Vec<u8>`** reads `TermMode::BRACKETED_PASTE` from the term, as `encode` reads DECCKM. Bracketed: `\x1b[200~` + text without ESC + `\x1b[201~`. Otherwise the bytes as is.

- **`C-S-v` too**, for Linux, where the desktop takes Super; it joins `editor_key`'s list, since char mode would otherwise send it as `^V`. `C-v` still reaches the shell.

## Risks / Trade-offs

- [Without bracketed paste, a multi-line paste runs line by line] → that is what the issue asks; programs that care turn on 2004.
