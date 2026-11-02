# Proposal

## Why

The M6 session design needs file chips drawn as rounded boxes (frames 6, 6b, 6c) and timestamps and durations held against the window's right edge (frame 6). The display model has neither, and both tasks that use them build on this one (#362).

## What Changes

- Faces gain a box colour, `box = 0xRRGGBB` in a theme, drawn as a 1px rounded box around the run without changing its width in cells. The bundled `attachment` face gets one.
- A new text property on a space char draws it as padding that puts the rest of the line against the window's last column, as Emacs's `(space :align-to right)`, and leaves it one space when the line doesn't fit. The buffer's text does not change, so motion, search, kill and copy see the one space.

Using either in the session buffer is out of scope: chips, timings and tool groups are their own M6 tasks. Neither draws in the minibuffer's candidates, mode lines or the echo line.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `display-model`: spans carry the box colour, and a new right-alignment requirement.
- `frontend-rendering`: boxed runs are drawn around their cells.

## Impact

- `crates/aiditor-core/src/buffer.rs`: `Face`, `Prop`.
- `crates/aiditor-core/src/display.rs`: `build`, `wrap_lines`, `Line`.
- `crates/aiditor/src/element.rs`: `line_runs`, row painting.
- `crates/aiditor-core/settings.toml`: the `attachment` face.
- `docs/editor.md`: the session display model.
- Existing user themes keep working; `box` is optional.
