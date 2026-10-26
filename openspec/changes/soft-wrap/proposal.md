## Why

Long lines are cut at the window edge, so a session buffer's streamed replies and a Markdown paragraph are unreadable without scrolling sideways. M3's session buffer needs soft wrap (#290, epic #112).

## What Changes

- The display builder splits each long buffer line into visual rows at the window's width. Session buffers and `.md`, `.markdown` and `.txt` files break at word boundaries; every other buffer breaks at the edge and shows a `\` continuation marker in the last column.
- Wrapping is on by default. `toggle-truncate-lines` switches the current buffer between wrapping and truncation with the existing horizontal scroll. The minibuffer always truncates.
- A window's first shown position becomes a visual row (a buffer line and a row within it), and keeping point visible counts visual rows.
- Only the rows that can be shown are built, so a long line costs its chars up to the window's bottom, not its whole length.

## Capabilities

### New Capabilities

### Modified Capabilities
- `display-model`: long lines wrap into visual rows by default, `toggle-truncate-lines` switches a buffer to truncation, and the horizontal scroll applies only to buffers that truncate.
- `windows`: scrolling to keep point visible counts visual rows.

## Impact

`aiditor-core`: `display.rs` (wrapping), `window.rs` (visual-row top), `frame.rs` (passing the wrap), `buffer.rs` (per-buffer truncate flag), `minibuffer.rs`, `builtins.rs`; `docs/editor.md`. The frontend draws the rows it is given and needs no change.
