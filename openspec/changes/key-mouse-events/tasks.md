# Tasks

## 1. Core

- [x] 1.1 Add the super modifier to `Key`, `parse_keys` and `Display` (`s-` after `S-`); verified by the round-trip test in `key.rs` including `C-s-a s-z`.
- [x] 1.2 Bind `s-z` to `undo` and `s-s` to `save-buffer` in the bundled keymap; verified by a harness test that `s-z` undoes typing and `z` does not.
- [x] 1.3 Add `option_as_meta` (default true) to `Settings` and the bundled `settings.toml`; verified by the settings default test.
- [x] 1.4 Add `Msg::Insert { text, replace }`: validate and convert the UTF-16 range, delete it, put point at its start, then feed each char through the key path; verified by harness tests for press-and-hold, a non-BMP char before the range, a bad range, `C-x u` and `C-u 3 a` through `Insert`.
- [x] 1.5 Make the harness send plain chars of key scripts as `Insert`; verified by the existing harness tests still passing.
- [x] 1.6 Add `Msg::Scroll(lines)` and `Msg::Click(char)` as the design says; verified by harness tests for scrolling with point kept visible, silent ends, click moving point, deactivating the mark, cancelling a prefix and starting a new undo group.

## 2. Frontend

- [x] 2.1 Replace `to_keys` with `to_key(&Keystroke, option_as_meta)` and consume only the keys it returns; verified by the existing key tests, updated, plus Cmd and Option-as-characters cases.
- [x] 2.2 Implement `EntityInputHandler` for `Editor` with marked text held in the frontend and drawn underlined at point, registered from a canvas that stores the text area's bounds; verified by GPUI tests for typing, a dead-key compose and press-and-hold.
- [x] 2.3 Add wheel and left-click handlers that accumulate scroll pixels into lines and hit-test clicks with the text system; verified by GPUI tests for a scroll and a click on a char.
- [x] 2.4 Add a GPUI test that presses every bundled keymap sequence as GPUI keystrokes and checks the core's key recording equals the parsed sequences.

## 3. Docs and checks

- [x] 3.1 Record in `docs/editor.md` the `option_as_meta` setting, that marked text is in the frontend until #11, the deferred `s-` bindings, and the mouse messages; remove the `to_keys` shortcut comment. Verified by reading the Keys from the frontend section.
- [x] 3.2 `cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D warnings` and `cargo test --workspace` pass.
