# Proposal

## Why

The frontend still draws the skeleton: only the selected window, lines as div text split around a cursor div (so moving point nudges glyphs), no region, no other windows of a split, and the hint on one clipped line. The core already builds a display model for every window (#35), so the frontend can now draw it as designed (issue #11).

## What Changes

- The frontend paints the frame display: every window of the split tree at its rectangle, each display line shaped once as a whole with one styled run per span (faces and the region), line decorations, and a mode line per window.
- The cursor is a bar painted over point's shaped line at the x offset of point's column; the line's text is never split, so moving point never moves glyphs. Only the selected window shows it.
- Input-method marked text is shaped into point's line, underlined, with the cursor after it.
- An echo area line is always at the bottom of the frame; the which-key hint is laid out in columns above it.
- The frontend reports the windows area's size and the font's cell width to the core with a new resize message; the core keeps them and lays the frame out from them, snapping split edges to whole lines and cells.
- Clicks and wheel scrolls go to the window under the pointer: a click selects it, a scroll scrolls it without selecting it. **BREAKING** (internal API): `Msg::Click` and `Msg::Scroll` name the window, and `Core::frame` takes no size.
- The theme gains mode line colors for the selected and other windows.
- The `--debug` stats line shows the last frame's shaping time.
- Not in scope: the minibuffer (the core has none yet), a cursor in unselected windows, tabs and wide chars as display columns, horizontal scrolling.

## Capabilities

### New Capabilities
- `frontend-rendering`: how the frontend draws the frame display: windows, shaped lines, cursor, region, mode lines, echo area, hint, debug stats.

### Modified Capabilities
- `display-model`: the frame display comes from the size the frontend reports, splits snap to whole lines and cells, and each window carries its id.
- `frontend-input`: the wheel and clicks act on the window under the pointer; marked text is drawn inside point's shaped line.
- `windows`: the "splitting a split" sizes are snapped to whole lines.
- `settings`: the theme gains mode line colors.

## Impact

- `crates/aiditor/src/main.rs`: a custom GPUI element replaces the div-based drawing; mouse handlers route by window.
- `crates/aiditor-core`: `Msg::Resize`, `Msg::Click`/`Msg::Scroll` with a window, stored frame size, `Core::frame()`, snapped `Tree::layout`, `FrameWindow::id`, theme keys in `settings.toml`; harness tests and snapshots updated.
- `docs/editor.md`: note that the frontend now draws the display model.
