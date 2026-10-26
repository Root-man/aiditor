## Context

`display::build` walks a window's buffer lines from its top line and emits one display line per buffer line (folds join lines), cut to the columns shown (`shown`); point's line is scrolled sideways by `Window::hscroll`. `Window::scroll` keeps point's buffer line among `rows`. The frontend draws whatever display lines it gets, one row each, so wrapping is entirely a core change.

## Goals / Non-Goals

**Goals:** wrap in the core display model; keep the existing truncation path untouched for buffers that truncate; bound the cost to what can be shown.

**Non-Goals:** `C-n`/`C-p` by visual line (Emacs's `line-move-visual`), `C-v`/`M-v`/`C-l` and jump centring by visual rows, a prefix argument for `toggle-truncate-lines`, a `truncate-lines` setting, pixel-measured widths (a column stays one char).

## Decisions

- **Rows hold the width less one char.** The last column is for the `\` marker in code, and for the cursor at the end of a full row in word wrap, as Emacs does on a terminal frame. A word-wrapped row may also put one whitespace char there, so a space at the width does not start the next row. Alternative: full-width rows with a frontend-drawn fringe marker; it needs frontend work and the cursor would still need somewhere to go.
- **Wrap after building.** `build` takes an optional wrap (`width`, `word`, `skip` rows of the top line). When wrapping it builds the unwrapped display lines with columns `0..(skip + rows + 1) * cols`, then splits each line's spans at its row starts, maps the cursor, appends the marker span, drops the first `skip` rows and keeps `rows`. Folds, faces, highlighting and the region come from the existing loop unchanged. Building wrapped rows inside the char loop was the alternative; it would tangle the fold and hscroll logic for no gain.
- **One row-break function.** `display::row_starts(len, width, word, is_space)` gives each row's starting column; the builder calls it on a display line's chars, `Window::scroll` on a buffer line's rope slice. Edge breaks are arithmetic, so a long code line costs `len / width` steps.
- **The window's top is (line, row).** `Window` gains `top_row` and `cols` (set by `window_display` from the last redisplay, as `rows` is). `scroll` finds point's row, walks `rows - 1` rows back from it for the lowest allowed top, and clamps the (line, row) pair between that and point's row. `set_top` and `jump` reset the row to 0; `scroll` still returns the top line, so `scroll_lines` and the tests keep working.
- **Policy on the buffer.** `Buffer::truncate_lines` (false by default) and `Buffer::wrap(cols) -> Option<(width, word)>`, word wrap for `agent-session-mode` and `.md`, `.markdown`, `.txt` files, and `None` for `minibuffer-mode` whatever the flag says.

## Risks / Trade-offs

- [Scrolling measures buffer chars, the display measures display chars] → with folds the two differ; hidden markup only makes scroll overestimate a line's rows, which keeps point visible. Noted as a shortcut, like the other fold shortcuts.
- [Word-wrapped lines above point are scanned whole when scrolling] → a long Markdown paragraph costs its length per redisplay; code lines are arithmetic.
- [A window deep inside one huge wrapped line lays out the chars before it] → cost grows with how far into the line the window is; same as Emacs.
