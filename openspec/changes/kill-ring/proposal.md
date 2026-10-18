# Proposal

## Why

Killed text is thrown away today (`M-d`/`M-DEL` carry a `shortcut:` until the kill ring lands), and there is no copy, cut or paste. Issue #31 adds Emacs's kill ring and yank, shared with the system clipboard as #30 decided.

## What Changes

- A kill ring of 60 entries in the core, newest first, with a yank pointer.
- New commands `kill-line` (`C-k`), `kill-region` (`C-w`, `s-x`), `kill-ring-save` (`M-w`, `s-c`), `yank` (`C-y`, `s-v`) and `yank-pop` (`M-y`).
- `kill-word` and `backward-kill-word` now save what they delete to the ring.
- Consecutive kills append to one entry, killing backward prepends.
- Clipboard sync: every kill writes the ring's newest entry to the system clipboard as an effect; before a kill or yank the frontend hands the core the clipboard's text, and text the editor did not write itself goes onto the ring first.
- `ArgSpec` gains a `clipboard` flag, and `Core::wants_clipboard(key)` tells the frontend whether the next key runs such a command, so the clipboard is read only when it is needed.

## Capabilities

### New Capabilities
- `kill-ring`: the kill ring, the kill and yank commands, appending, and clipboard sync.

### Modified Capabilities
- `editing-keys`: `M-d` and `M-DEL` kill to the ring instead of dropping the text.

## Impact

- `crates/aiditor-core`: `lib.rs` (ring, commands, `Msg::Clipboard`, `Effect::SetClipboard`, `wants_clipboard`), `command.rs` (`ArgSpec::clipboard`), `keymap.toml`, the harness (a fake clipboard) and its tests.
- `crates/aiditor/src/main.rs`: reads the clipboard before a key that wants it and writes it on `Effect::SetClipboard`.
- `docs/editor.md`: notes on what was built.
- No new dependencies.
