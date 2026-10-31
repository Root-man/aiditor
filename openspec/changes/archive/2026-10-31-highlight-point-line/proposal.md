# Proposal

## Why

Nothing marks point's line but the block cursor and its number, so point is slow to find in a full window. The approved design draws the line in `hl-line`, the face already exists in the theme, and nothing uses it yet (#354).

## What Changes

- Point's line gets the `hl-line` background across the window, gutter included, in file buffers, `*scratch*`, `*Messages*` and terminal copy mode, as Emacs's `hl-line-mode`.
- Only one window has it: the selected one, or while the minibuffer is open the window it was opened from, as frame 10 draws swiper's preview.
- The region and any face with a background draw over it.
- A new `hl_line` setting, bundled as `true`, turns it off.

## Capabilities

### New Capabilities
- `line-highlight`: the background on point's line, where it shows and the setting that turns it off.

### Modified Capabilities

None.

## Impact

`aiditor-core`: `settings.rs`, `settings.toml`, `display.rs`, `frame.rs`, `minibuffer.rs` and the frame snapshots. The frontend already paints line backgrounds under text. `docs/editor.md` records the behavior.
