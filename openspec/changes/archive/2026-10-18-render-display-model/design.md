# Design

## Context

See proposal.md for why. `Core::frame(width, height)` already returns every window's rectangle, selection and display (lines of styled spans with the region baked into span backgrounds, decorations, cursor as line and char column, mode line text). The frontend ignores it and draws the selected buffer as div text. The GPUI spike (`examples/gpui_spike.rs`, docs/spikes/gpui.md) settled the drawing pattern: a custom `Element` that shapes each line with `shape_line` and paints backgrounds, text and a cursor quad, with `x_for_index` answering positions.

## Goals / Non-Goals

**Goals:** draw exactly the frame display; never split a line's text around the cursor; route mouse input to the window under the pointer; give the core the real frame size and cell width.

**Non-Goals:** reshaping only changed lines (GPUI's line layout cache already reuses last frame's layouts for identical text and runs); a cursor in unselected windows; the minibuffer; display columns for tabs and wide chars.

## Decisions

- **One `FrameElement` for all windows.** It fills the space between the stats line and the hint/echo area. In `prepaint` it measures its bounds and the font's cell width (`ch_advance`), sends `Msg::Resize` through `Editor::handle` when either changed, calls `core.frame()`, and shapes every display line and mode line, timing that as the shaping time. In `paint` it registers the input handler and, per window inside a content mask of its rectangle, paints decoration backgrounds, line backgrounds, text, rules, the cursor quad and the mode line. Alternative: one `StyledText` div per line with an absolutely placed cursor; rejected because the cursor's x needs the line's layout before paint, which a custom element has in `prepaint`.
- **One `TextRun` per span**, color from the face or the theme foreground, background from the face. Point's line is shaped like every other line; the cursor is a 2 px quad at `x_for_index(byte of the cursor column)`.
- **Marked text** stays in the frontend (it is platform input state, not core state). It is spliced into point's line text before shaping as an extra underlined run, and the cursor goes after it.
- **The frontend keeps the last frame's layout** per window: its id, rectangle, line height and `(buffer line, ShapedLine)` per row, plus the cursor's bounds. Clicks and scrolls hit-test it; `bounds_for_range` returns the cursor's bounds.
- **`Msg::Resize { width, height, cell_width }`** stores the size in the core; `Core::frame()` uses it. The frontend sends it from `prepaint` only on change, so the cost is one extra frame per resize. Alternative: keep `frame(width, height)` with a cell width argument; rejected because the issue asks for the core to be told on resize, and commands can then rely on the stored size later.
- **`Tree::layout(rect, cell)` snaps split edges**: a stacked split's first part is its ratio of the height rounded to whole line heights, a side-by-side split's to whole cell widths; the second part takes the rest. A zero cell (no resize reported yet) means no snapping.
- **Mouse messages name the window.** `FrameWindow` gains `id: WindowId`. `Msg::Click { window, line, col }` selects the window, then moves point to `char_idx(line, col)`, the buffer's end when the line is past it; the frontend computes the column from `closest_index_for_x` on the clicked row. `Msg::Scroll { window, lines }` selects the window, scrolls, and reselects the previous one. Clicks on a mode line or outside any window do nothing.
- **Echo area and hint are divs** below the element: the echo area is always one line; the hint is a wrapping flex row of entries each one column wide, the column being the longest entry plus two cells.
- **Theme keys** `mode_line` and `mode_line_inactive` in `settings.toml`.

## Risks / Trade-offs

- [Display columns differ from buffer columns inside folds] → the click column is mapped as a buffer column; `shortcut:` comment, revisit when folds have producers.
- [Sending `Resize` during prepaint triggers a second frame] → only on size or font change.
- [Shaping every visible line every frame] → measured by the new stats; GPUI's frame-to-frame layout cache keeps unchanged lines cheap. Check scrolling `docs/editor.md` with `C-v` stays well under 35 ms per frame in a debug build.
- [Tabs render as whatever the font gives `\t`] → unchanged from the skeleton.
