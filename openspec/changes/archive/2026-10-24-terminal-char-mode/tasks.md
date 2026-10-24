# Tasks

## 1. Keys

- [x] 1.1 Add Home, End, PageUp, PageDown, Delete and F1–F12 to `KeyCode`, with `kbd` names in `parse_keys` and `Display`; verify the round-trip test includes them
- [x] 1.2 Map `home`, `end`, `pageup`, `pagedown`, `delete`, `f1`–`f12` in the frontend's `to_key` and the GPUI test's `keystroke`; verify `to_key` tests for `f1` and `shift-home`

## 2. Char mode

- [x] 2.1 Add `Terminal::encode` per the spec; verify unit tests of the bytes for chars, control keys, meta, special keys, function keys and modified keys in both cursor modes
- [x] 2.2 Route keys and typed text of a live terminal to `TerminalOp::Write` in `Ctx::key` / `Msg::Insert`, except `C-x`, `C-c`, `M-x` and super keys; verify harness tests for typing `echo hi RET`, readline keys, `C-x b`, `M-x` into the minibuffer, and DECCKM arrows
- [x] 2.3 Give terminal buffers major mode `vterm-mode`, add `vterm-send-C-c` and bind `C-c C-c` in `[vterm-mode]`; verify a harness test that `C-c C-c` writes `\x03`
- [x] 2.4 Update the read-only text test to run commands with `M-x`; record char mode as built in `docs/editor.md`; run fmt, clippy and tests as CI does
