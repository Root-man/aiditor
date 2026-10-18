# Design

## Context

`crates/aiditor/src/main.rs` turns every keystroke into `Msg::Key` in `to_keys`, typed chars included, and never registers an input handler, so the platform's text input never runs. Cmd keys are dropped. The core has `Key { code, ctrl, meta, shift }`, `self-insert-command` reads the char from the last key, and `scroll_lines` already scrolls while keeping point visible (used by `C-v`). Rendering is the skeleton's divs, which #11 replaces.

GPUI 0.2.2 facts this relies on: with nothing in its keymap, every key reaches `on_key_down`; a key that propagates goes to the registered input handler (`replace_text_in_range(None, key_char)`) on macOS, Wayland, X11 and the test platform; while the handler reports marked text, macOS sends keys to the IME first. Arrow keys on macOS do not set `function`. The test platform's `NoopTextSystem` gives every glyph the same width, so hit-testing can be tested.

## Goals / Non-Goals

**Goals:** every bundled binding reaches the core on both platforms; typing, dead keys and press-and-hold go through the platform's text input; wheel scroll and left click as #30 decided.

**Non-Goals:** Fn keys, F-keys, Home/End/PageUp/PageDown/forward Delete (no `KeyCode` for them yet); `s-c`/`s-x`/`s-v` (#31) and `s-q`/`s-w`/`s-h` (m0-quit); drag, double click, PRIMARY; moving marked text into the core's display state (#11); `catch_unwind` around core calls (the panic issue); IME candidate-window placement beyond the cursor's line box.

## Decisions

- **Super is a fourth bool on `Key`** (`super_`, written `s-` after `S-`, Emacs's order). It keeps `Key` `Copy` and `Hash` and every `..Key::plain` literal compiles unchanged.
- **Key classification is one function, `to_key(&Keystroke, option_as_meta) -> Option<Key>`.** `None` means "let it propagate". Meta is `alt && (option_as_meta || !macOS)`. Named keys map as before. With Ctrl, Cmd or Meta held, or when `key_char` is `None`, a one-char key name (`space` as `' '`) becomes a char key; anything else is `None`. Fn held is `None`, as before: `Key` has no Fn and Fn-letter would otherwise self-insert.
- **`Msg::Insert` reuses the key path.** Each char of `text` is fed to the same function `Msg::Key` uses, as `Key::plain(Char(c))`. That keeps prefixes, transient digit maps, mode keymaps, prefix-argument repeats, undo amalgamation and macro recording correct with no new code paths. The alternative, a separate insert path, would have had to reimplement all of these. A `replace` range is converted with ropey's `utf16_cu_to_char`, checked against the buffer (start <= end <= length in UTF-16), deleted, and point is put at its start before the chars run; a bad range is reported on the message line and nothing changes.
- **The harness sends plain chars as `Insert`.** In `Harness::keys` a key with no modifier and a `Char` code becomes `Msg::Insert { text, replace: None }`, as #9 decided.
- **Marked text lives in the frontend `Editor`** (`marked: Option<String>`), not in the core. The decision put it in the window's display state, but the frontend does not draw from the display model until #11; putting it in the core now would mean a message and display fields nobody draws. A `shortcut:` comment points #11 at moving it. The input handler presents the IME a virtual document, the buffer with the marked text spliced in at point; offsets past point are shifted by the marked length when converted to buffer ranges. `unmark_text` commits the marked text as `Insert`; a replace that covers the marked range clears it and inserts the new text.
- **Mouse messages are `Msg::Scroll(i64)` and `Msg::Click(usize)`** (char index). The frontend owns geometry: it accumulates `ScrollWheelEvent::delta.pixel_delta(line_height)` and sends whole lines; for a click it maps y to a line from the text area's top, shapes that line with the window's text system and takes `closest_index_for_x`. The core keeps hit-testing out, as the architecture says layout is GPUI's. `Scroll` calls `scroll_lines` and drops its end-of-buffer error. `Click` clears the pending sequence, transient map, prefix argument, hint and message line (as a click clears the echo area in Emacs), resets the self-insert run, moves point clamped to the buffer, deactivates the mark and sets the last command to `mouse-set-point`.
- **The input handler is registered from a `canvas` over the text area**, which also stores the area's bounds for clicks and `bounds_for_range`, the pattern of GPUI's `examples/input.rs`.
- **Frontend tests use GPUI's test platform** (`gpui` with `test-support` as a dev-dependency). One test types every bundled keymap entry as GPUI keystrokes, with the core recording, and compares the recording with the parsed sequences.

## Risks / Trade-offs

- Real IMEs (Japanese, Linux text-input-v3) are untested; only the dead-key and press-and-hold flows are exercised, through the input handler in tests and by hand on macOS.
- An IME commit with a prefix pending feeds its chars as keys, so `C-x` then a committed `日本` reports `C-x 日 is undefined` and types `本`. Emacs behaves the same way.
- The frontend's virtual document builds strings from rope slices on each IME query; queries are short ranges near point, so this is cheap.
