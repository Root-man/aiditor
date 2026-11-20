# Design

## Where the rows live

`Window.diff_rows: Vec<DiffRow>`. A `DiffRow` holds a marker at its anchor line's start (`InsertionType::Advance`, so text typed at the line's start keeps the row with the line's old text), `after: bool`, its runs `Vec<(String, Vec<FaceRef>)>` (the type `Prop::Display` uses, so a theme change recolours them), `label: Option<String>` and `height: usize`. `Window::set_diff_rows` replaces them; `release` removes their markers, so showing another buffer (which rebuilds the window) and deleting the window drop them. `split` builds from `Window::new`, so a split gets none.

## Rows in scrolling

Scrolling works in `(line, row)` pairs. A line's rows become a *block*: the rows of diff rows drawn above its text (those after the line before and those before it), then its text rows. With point on the last row of the last line, `scroll` keeps the rows after that line in view too, as long as point's row stays shown. `top_row` and `back` index the block, so scrolling and `C-l` count diff rows, while `row_starts` stays the text rows motion moves through. A diff row takes `max(height, wrapped rows)` rows. A top row inside the above rows starts the text from column 0 and drops that many rows from the splice.

## Rows in the display

`build` stays as it is; `frame::window_display` splices after it. Each first row of a buffer line gets the line's above rows before it (less those scrolled off for the top line); the last line's last row gets its below rows after it when the buffer's end is shown. The cursor's index moves down by what was spliced before it, and the lines are cut back to the rows drawn. A diff row is one `Line` per row, with `diff: Some(label)` (the label on its first row, empty after), its runs as spans, wrapped through `wrap_lines` or cut to the width, padded with empty rows to its height. `gutter` draws a diff row's label in `line-number`; `highlight_line` skips diff rows. `C-l` places point's row with `back`, so it counts them; `C-v`/`M-v` still scroll buffer lines, as over wrapped lines. `Core::set_diff_rows` sets the selected window's rows. The frontend draws them as any line.

## Not done

No command makes rows yet (#505 does). No background for a row's full width, which the inline diff may add on `DiffRow`. Session windows don't splice rows.
