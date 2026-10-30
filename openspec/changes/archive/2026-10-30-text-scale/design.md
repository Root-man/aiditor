# Design

## Context

Every drawing path reads `Settings::font_size` or `line_height()`, and the frontend's prepaint already notices a changed cell width and sends `Msg::Resize`, as a reload does. `Core::settings` is replaced whole when settings load.

## Goals / Non-Goals

**Goals:** zoom text and everything sized from it, on the platform's keys, without a frontend change.

**Non-Goals:** scaling fixed spacing (chrome padding, gutter margins, cursor and rule widths), saving the zoom, per-buffer zoom, numpad keys, the mouse wheel, prefix arguments on the zoom commands.

## Decisions

- **Font size, not a window scale.** GPUI has no production scale override: `Window::set_scale_factor` is test-only and `bounds_changed` resets it, and `set_rem_size` only affects `rems` while aiditor draws in `px`.
- **`font_size` stays the effective size; a skipped `default_font_size` holds the file's value.** Readers and tests keep working unchanged, against a separate offset every reader would have to add. The offset on reload is `font_size - default_font_size`, so it needs no state of its own.
- **1px steps.** Readable at both ends of a 6 to 72 range and simple to reason about; a ratio like Emacs's 1.2 leaves fractional sizes.
- **Bound in the platform keymaps,** as the clipboard keys are, because Linux's desktop owns Super. Both `=` and `+` are bound: GPUI reports Ctrl+Shift+`=` as `+` without Shift.
- **`M--` for `negative-argument`.** Emacs binds it too, and it is free. It goes in the shared keymap, so tests and muscle memory match on both platforms; macOS keeps `C--` as well because nothing there displaces it.

## Risks / Trade-offs

- [`C--` changes meaning on Linux] → recorded in the spec, `M--` replaces it.
- [The zoom keys no longer reach terminal programs on Linux] → they rarely mean anything there, and terminals like Ghostty take them too.
- [#353 adds `workspace_font_size`, a second size] → it must apply the same offset when it lands; noted in `docs/editor.md`.
