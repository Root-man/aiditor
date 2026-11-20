# Tasks

## 1. Core

- [x] 1.1 `DiffRow` and `Window.diff_rows` with `set_diff_rows`, markers released with the window's
- [x] 1.2 Scrolling: a line's block of rows (above rows, text rows, the last line's below rows) in `scroll`, `back`, `shown`; `top_row` indexes it
- [x] 1.3 Display: `Line.diff`; splice the rows after `build` in `window_display`, wrapped or cut, padded to height, cursor index moved; gutter labels; `hl-line` and the message bar skip them

## 2. Frontend

- [x] 2.1 The new `Line` field in the frontend's tests; a GPUI test that rows paint in both themes

## 3. Tests and docs

- [x] 3.1 Core tests: placement and labels, filler, wrapping, motion and copying skip rows, scrolling counts them, two windows, edits keep anchors, removing restores the display
- [x] 3.2 `docs/editor.md`: diff rows as built
