# Proposal

## Why

The skeleton consumes every key in `on_key_down`, typing included, so dead keys, macOS press-and-hold and IMEs cannot work, Cmd keys never reach the core and the mouse does nothing. #9 decided how keys split into commands and typing, and #30 put wheel scrolling and left click in the MVP; this change wires both up (issue #12).

## What Changes

- Keys gain the `s-` (super, Cmd) modifier in `kbd` notation, and the bundled keymap binds `s-z` to `undo` and `s-s` to `save-buffer`. `s-c`/`s-x`/`s-v` come with the kill ring (#31) and `s-q`/`s-w`/`s-h` with quitting (m0-quit), since quitting without its unsaved-buffers prompt would lose work.
- The frontend sorts each key into a command (Ctrl, Cmd, Meta, RET/TAB/DEL/ESC, arrows, or a key that types nothing) sent to the core as a key, or typing that goes to the platform's text input and arrives as `Msg::Insert { text, replace }`.
- A new `option_as_meta` setting (default `true`, macOS only) chooses Option as Meta or Option for characters and dead keys. On Linux Alt is always Meta.
- The core handles `Insert`: an optional UTF-16 `replace` range is validated, converted and deleted, then each char of `text` goes through the key path like a typed plain char. Key scripts in the harness send their plain chars as `Insert`s.
- IME marked text is held and drawn by the frontend, underlined at point, and never enters the buffer; committing it inserts it.
- Mouse: the wheel or trackpad scrolls the window by lines, keeping point visible as `C-v` does. A left click moves point to the clicked char, deactivates the mark and cancels a pending prefix.

## Capabilities

### New Capabilities
- `frontend-input`: how key, text-input, IME and mouse events from the platform reach the core.

### Modified Capabilities
- `keymaps`: key sequences can name the `s-` modifier, and the bundled keymap binds `s-z` and `s-s`.
- `settings`: settings gain `option_as_meta`.

## Impact

- `aiditor-core`: `key.rs` (super modifier), `lib.rs` (`Msg::Insert`, `Msg::Scroll`, `Msg::Click`), `settings.rs` and `settings.toml`, `keymap.toml`, `harness.rs`.
- `aiditor` frontend `main.rs`: key classification, an `EntityInputHandler`, wheel and click handlers, marked-text drawing; GPUI's `test-support` feature as a dev-dependency for frontend tests.
- `docs/editor.md`: where marked text lives for now, the deferred `s-` bindings, the setting's name.
