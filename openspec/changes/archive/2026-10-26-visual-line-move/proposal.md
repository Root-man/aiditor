## Why

Wrapped lines (#290) show as several rows, but `C-n` and `C-p` still jump a whole buffer line, so a long paragraph in a session buffer or a Markdown file can't be walked row by row. Emacs moves by visual lines (`line-move-visual`) by default (#291, epic #112).

## What Changes

- `next-line` and `previous-line` move by visual rows in a buffer that wraps, keeping the goal column within the row: column `c` of a row lands on column `c` of the target row, clamped to that row's last char (or to the line's end on its last row).
- A count moves that many rows; running into either end of the buffer moves there and reports it, as now.
- Truncating buffers (`toggle-truncate-lines`) and the minibuffer still move by buffer lines.
- `C-a`, `C-e` and `C-k` keep acting on buffer lines, as in Emacs without `visual-line-mode`'s rebinds.

## Capabilities

### New Capabilities

### Modified Capabilities
- `motion`: line motion moves by visual rows when the buffer wraps, with the goal column within the row.

## Impact

`aiditor-core`: `motion.rs` (`next_line`), `window.rs` (row starts shared with `scroll`); `docs/editor.md` (the soft-wrap "Not yet" and the motion entry). The frontend needs no change.
