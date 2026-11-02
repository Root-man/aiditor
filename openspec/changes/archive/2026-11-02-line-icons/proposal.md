# Proposal

## Why

Frame 6 draws small line icons in tool rows, and a font can't be relied on to have them. Issue #416 adds the means; the tool-header and tool-group tasks, which it blocks, use it.

## What Changes

- A text property names an icon from a bundled set (book, magnifier, pencil, terminal, globe, wrench) on a range of two plain spaces. The text stays those spaces, so motion, search, kill and copy see nothing else.
- The display model says where each icon sits on its row.
- The frontend paints the icon as an SVG over the two cells, in the foreground of the run under it.
- The set is drawn from Lucide, whose ISC licence allows bundling; the licence ships beside the files and `docs/editor.md` records it.

No session row shows an icon yet: that is the tool-header task's.

## Capabilities

### New Capabilities

### Modified Capabilities
- `display-model`: the icon property and where the display model places icons.
- `frontend-rendering`: painting icons from the bundled set.

## Impact

`crates/aiditor-core/src/buffer.rs`, `display.rs`, `crates/aiditor/src/element.rs`, `main.rs`, a new `icons.rs`, `crates/aiditor/resources/icons/`, insta snapshots that print display lines, `docs/editor.md`.
