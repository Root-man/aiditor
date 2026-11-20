# Proposal

## Why

The inline diff view (#505) draws a file's removed lines between the lines of its own buffer, and the side-by-side view after it puts filler under either side. Both need rows in a window's display that are not the buffer's text (#533).

## What Changes

- A window can hold *diff rows*: rows anchored before or after a buffer line, each with its own text runs and faces, its own gutter label (an old line number or none) and a height in lines; a row with no text is filler.
- The display splices them in where they are anchored: before the first line, between two lines, after the last line. They take their own gutter label, never the cursor, `hl-line`, the region or search highlights. A row wider than the window wraps as text lines do, or is cut where they are.
- Scrolling, `C-l` and the visible line count count their rows; point, motion, search, the region and copying skip them, since they are not text.
- They belong to the window: the same buffer in another window shows none, a split copies none, and showing another buffer drops them. Edits keep each one at the line it is anchored to.

## Capabilities

### New Capabilities
- `diff-rows`: rows drawn between a buffer's lines in one window.

### Modified Capabilities

## Impact

`aiditor-core`: `window.rs` (rows and their anchors, scrolling counts them), `display.rs` (`Line.diff`, splicing, the gutter), `frame.rs` (where a window's display is built). `aiditor`: none beyond the `Line` field; a test that the rows paint. Snapshots gain the field. `docs/editor.md`.
