# Proposal

## Why

Terminal buffers show the shell ([#225](https://github.com/Root-man/aiditor/issues/225)) but keys still run editor commands, so nothing can be typed into them. Issue [#232](https://github.com/Root-man/aiditor/issues/232) adds char mode, with the keys that stay with the editor as [#89](https://github.com/Root-man/aiditor/issues/89) decided.

## What Changes

- A live terminal buffer is in char mode: every key except `C-x`, `C-c`, `M-x` and Cmd keys is encoded as xterm does and written to the shell instead of running a command. `C-x`, `C-c`, `M-x`, Cmd keys and the sequences under the prefixes go through the keymaps. A `vterm-mode` keymap binds `C-c C-c` to `vterm-send-C-c`, which writes `^C`.
- Encoding follows Zed's `keys.rs`: chars as UTF-8, `C-<char>` as its control byte, `M-` as an ESC prefix, `RET` `\r`, `DEL` `\x7f`, `S-TAB` `\x1b[Z`, arrows, Home and End in normal or application cursor mode (DECCKM), modified cursor and function keys as `\x1b[1;<m>X` / `\x1b[<n>;<m>~`.
- `Key` gains Home, End, PageUp, PageDown, Delete and F1–F12 (`<home>`, `<end>`, `<prior>`, `<next>`, `<delete>`, `<f1>`…), and the frontend reports them.
- Text from the platform's input (typed chars, Option-typed chars, IME commits) is written to the shell whole.
- Defaults picked: a key with no xterm encoding (like `C-1`) is dropped; an IME's replace range is ignored in a terminal; the new keys are unbound in text buffers, so they report `is undefined`.
- Not in scope: copy mode (#88), a send-next-key command, bracketed paste, mouse reporting.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `terminal-buffers`: char mode sends keys and text to the shell; text commands run only by name.
- `keymaps`: key sequences name Home, End, PageUp, PageDown, Delete and F1–F12.
- `frontend-input`: those keys reach the core as keys.

## Impact

- `aiditor-core`: `key.rs` (new key codes), `terminal.rs` (encoding, char-mode dispatch, `vterm-send-C-c`), `lib.rs` (`Ctx::key` and `Msg::Insert` route to the terminal), `buffer.rs` (terminal buffers' major mode), `builtins.rs`, `keymap.toml`.
- `aiditor` frontend: `input.rs` maps the new keys; the bundled-binding GPUI test names them.
- `docs/editor.md`: the terminal paragraph records char mode as built.
- No new dependencies.
