# Design

## Context

See proposal.md, Why. The current code at the time of writing:

- `display::build` (`crates/aiditor-core/src/display.rs:83-188`) walks the buffer from line `top` one char per loop turn. A `shortcut:` comment above it (`display.rs:82`) says lines are emitted whole and the frontend clips them. The loop handles three cases. A fold (`Prop::Invisible`) containing `pos` emits its placeholder and jumps to its end (`display.rs:121-138`), which is how a fold over a newline joins lines (test `a_fold_joins_lines`). A `'\n'` calls `finish`, which gives the line every `Prop::Line` whose range touches `start..end` (`display.rs:107-118`). Any other char gets a face scan and a push.
- `window_display` (`crates/aiditor-core/src/lib.rs:2334-2366`) calls `build`, then calls `window.hscroll(col, cols)` with the cursor's display column and trims that many chars off the front of point's row only (`lib.rs:2351-2364`).
- `cols` is `(rect.width / cell_width) as usize` in `Core::frame` (`lib.rs:2259`), which is 0 before the first `Msg::Resize` (`frame_size` starts at `(0., 0., 0.)`, `lib.rs:1761`; 0/0 is NaN, which casts to 0). The minibuffer passes the frame's columns less the prompt (`lib.rs:2294`), and the test-only `Core::display` passes `usize::MAX` (`lib.rs:2239`). `Window::hscroll` returns 0 for 0 columns (`window.rs:177-185`).
- Clicks add `window.left()` back on point's buffer line (`lib.rs:843-853`). Other lines map from column 0.

## Goals / Non-Goals

**Goals:** `build` does work proportional to the shown columns per line, not the line's length. The model holds the columns the spec delta allows. Today's look, existing snapshots and the frontend test `the_cursor_is_drawn_over_the_whole_line` (`crates/aiditor/src/main.rs:1393`) stay unchanged.

**Non-Goals:** skipping the face scan for the chars that are shown (the brief's per-char costs stay for the visible width), indexing properties, and any frontend change.

## Decisions

### Settle the offset before `build`

`window_display` takes point's column from `buffer.line_col(point)` and calls `window.hscroll(col, cols)` before `build`. This is a char column in point's buffer line, while the old code used the display column. The two differ only when a fold sits on point's line before point, and nothing produces folds yet. It gets a `shortcut:` comment like the click mapping's (`main.rs:233`). The alternative, building twice or building point's line first, costs more code for a case that cannot happen yet.

### One range argument for `build`

`build` takes `shown: Range<usize>`, the columns of point's line: `left..left + width`. Other lines use `0..shown.len()`. A range keeps `build` at 7 arguments, under clippy's `too_many_arguments` limit, which a separate `left` and `cols` would cross. `build` knows point's line by comparing `line.number` with point's buffer line from `line_col`, so a display line joined by a fold that starts on an earlier buffer line is not shifted. That is the same fold shortcut as above.

`width` is `cols + 1`, using `saturating_add`, and is `usize::MAX` when `cols == 0`:

- **`cols + 1`, not `cols`.** The window rect is rarely a whole number of cells wide, and today the partly visible last column draws its glyph's left part. Emitting exactly `cols` chars would leave that cell blank, which is a visible change. The extra char costs one glyph per line. `hscroll` keeps `col - left <= cols - 1` (`window.rs:179-184`), so the cursor stays inside the emitted text either way.
- **`cols == 0` means no limit.** It matches `hscroll`'s "no columns, no scroll". Clipping at 0 would show empty lines before the first resize. `usize::MAX` from `Core::display` saturates, so its snapshots do not change.

### The loop

- A per-line counter `col` counts display columns consumed so far. Chars and placeholder chars both count, and skipped columns count too. It replaces the `col(&line)` sum. The cursor column is `col - left` on point's line and `col` elsewhere.
- **Before `left`** (point's line only), a char only bumps `col`. It gets no face scan and no push. A placeholder that straddles `left` or the limit is cut char by char, as the old post-cut did.
- **At the limit**, the turn jumps instead of reading a char. It goes to the earliest of two targets: the line's newline, which is `text.line_to_char(l + 1) - 1` for `l = text.char_to_line(pos)`, or `len_chars` on the last line; or the start of the first `Prop::Invisible` range that starts in `pos..newline`. The loop then goes on as today. A fold found at the top of the turn jumps to its end, with its placeholder not emitted while the line is full. That keeps a fold that hides the newline joining the lines. The joined line stays full, so the next turn jumps again within the new buffer line. The newline itself is still read by the normal path, so `finish` gets the line's real end and its decorations stay exact.
- The post-cut in `window_display` goes away. `build` already returns the cursor in shown coordinates.

The jump costs one scan of the properties per cut line, about the same as one char today.

### Spec delta rather than `skip_specs`

The spec's "Back to a column that fits" scenario said the line is "shown whole", and the harness test `point_s_line_scrolls_sideways_to_keep_point_visible` (`harness.rs:1926`) asserts whole 100-char lines in a 40-column window. Both change, so the bound goes into the requirement with one scenario that pins it.

## Risks / Trade-offs

- [Off by one at the edge] → The 10 000-char test checks exact text and the cursor at both ends. The existing `C-e`/`C-a` test is updated to `long[..41]`.
- [A fold that ends exactly at a line end, or starts inside the cut tail] → One test with a fold joining a long line to the next, cut at 20 columns, asserts the joined line and its number. `a_fold_joins_lines` and the other fold snapshots stay unchanged, since `Core::display` cuts nothing.
- [The per-char loop is the riskiest code to restructure] → All existing insta snapshots must pass unchanged. Any snapshot diff is a bug, not an update.
- [Tabs and wide chars count as one column] → Unchanged. The existing shortcut comment moves with the cursor logic.
