# Proposal

## Why

Point is a bare offset in the core and the scroll position lives in the frontend, so an edit made anywhere but at point leaves point in the wrong place, and scrolling is untested. The spec's window (buffer id, point, mark, scroll position) is what splits, the motion and mark commands and the display model all build on (issue #33).

## What Changes

- A window shows one buffer and holds point and mark as markers in that buffer, plus the first line it shows.
- The core has one window; the current buffer is the window's buffer. Visiting a file shows it in that window.
- The core keeps the scroll position and moves it only as far as needed to keep point's line visible, given how many lines fit; the frontend asks the core for it instead of keeping its own.
- Out of scope: commands that set the mark or show the region, goal column, `C-v`/`M-v`, splits, reporting the frame size to the core.

## Capabilities

### New Capabilities
- `windows`: viewports onto buffers: point and mark that follow edits, and a scroll position that keeps point visible.

### Modified Capabilities

## Impact

- `aiditor-core`: new `window` module; `Core` drops its `current` buffer id and `point` offset for one window.
- `aiditor` frontend: drops its own `top` and takes the scroll position from the core at render.
