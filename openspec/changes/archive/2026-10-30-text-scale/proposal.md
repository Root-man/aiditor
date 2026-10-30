# Proposal

## Why

There is no way to change the text size without editing `settings.toml` and reloading. Editors and terminals such as Ghostty zoom on the platform's `+`, `-` and `0` keys, which suits a keyboard-first editor moved between displays.

## What Changes

- `text-scale-increase`, `text-scale-decrease` and `text-scale-reset` change the frame's font size 1px at a time within the 6 to 72 range `font_size` allows, and reset it to the `settings.toml` value. Line height, mode lines, gutters and terminal grids follow the font size.
- `C-=`, `C-+`, `C--` and `C-0` on Linux and `s-=`, `s-+`, `s--` and `s-0` on macOS run them. `+` and `=` are both bound so Shift is optional.
- The zoom survives `C-x C-r` and lasts until the app exits; it is not saved.
- **BREAKING** On Linux `C--` no longer runs `negative-argument`. `M--` runs it on both platforms, as in Emacs; macOS keeps `C--` too.
- On Linux the four zoom keys stay with the editor in a terminal's char mode. On macOS super keys already do.

## Capabilities

### New Capabilities

- `text-scale`: changing the frame's text size at runtime from the keyboard.

### Modified Capabilities

- `prefix-argument`: `M--` replaces `C--` as the minus key.
- `editing-keys`: scenarios use `M--`.
- `motion`: a scenario uses `M--`.
- `terminal-buffers`: char mode leaves the zoom keys with the editor on Linux.

## Impact

`aiditor-core`: `settings.rs`, `builtins.rs`, `lib.rs`, `terminal.rs`, the three keymap files and tests using `C--`. `docs/editor.md` mentions `C--`; `docs/roadmap.yaml` only does for closed #23, which stays as written. The frontend needs no change: it reads the font size each frame and already handles a changed cell size. There is no GitHub issue for this change.
