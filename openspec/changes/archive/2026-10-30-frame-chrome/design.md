# Design

## Context

See proposal.md for the motivation. `Settings::line_height()` is the font size times a literal 1.4. `window_display` takes a pixel height and assumes the mode line is one line. `Tree::layout` tiles the windows with no gaps and snaps split edges to whole cells. `frame.rs`, `terminal.rs` and `minibuffer.rs` each derive rows or columns from those. The frontend paints mode lines one line high and insets the whole root by 8px.

The approved values are frame 12's; `docs/editor.md` settles the edge padding as 12px where frames disagree.

## Goals / Non-Goals

**Goals:**
- The core owns every number rows and columns depend on, so harness tests can check them.

**Non-Goals:**
- Mode line segments, `│` separators and `mode-line-dim` (#355, #356).
- `hl-line` (#354), the `echo` face on messages, the minibuffer's 4/6px padding, and every workspace in the bar (#360).
- Cascadia Code or Gruvbox as bundled defaults: `docs/editor.md` keeps them in the user's settings.

## Decisions

- **Sizes live on `Settings`.** `line_height()` stays the px height; the field `line_height` is the multiple. `mode_line_height()` and `workspace_modeline_height()` round as the spec says. Alternative: compute in the frontend, but the core needs the mode line's height for rows.
- **`workspace_font_size()` adds the zoom instead of storing it.** It is the setting plus `font_size - default_font_size`, clamped, so reload and reset need no new code.
- **`text_rows(height)` on `Settings` is the one row count.** `window_display` takes rows, not height, so the minibuffer passes 1 and stops pretending to be two lines high; `window_cells` uses the same function.
- **Constants in `display.rs`:** `TEXT_INSET` 12 and `WINDOW_TOP` 6, beside the gutter's. `FrameWindow` needs no new field: the frontend's `text_x` is the gutter's width or `TEXT_INSET`.
- **The right inset applies to every window**, gutter or not, as "text never touches the window edge". Frames 1 and 2 show no long lines at the edge, so nothing contradicts it.
- **Layout gets the chrome height.** Stacked splits snap the text area, `snap(h * ratio - chrome, line) + chrome`, otherwise every upper window would waste most of a line above its mode line. Side-by-side splits give the second part `a.width + DIVIDER` less.
- **The divider is painted by exclusion.** The element fills its bounds in `divider`, then each window in the default background, so the gap shows through. Alternative: a `dividers` list in `FrameDisplay`, more code for the same pixels, with the geometry already tested through the rectangles.
- **Chrome quads are prepainted into `Painted`** as a list of bounds and colour, so a frontend test reads the rule and the bar without a screenshot.
- **Ligatures:** `Editor::font` sets `FontFeatures::disable_ligatures()`, every run clones that font, and the root div takes it with `.font(...)` so div text inherits the features too.
- **`fit_names` scales the cell** by `workspace_font_size / font_size`, since a monospace advance is linear in size, and subtracts both insets.

## Risks / Trade-offs

- [Rounded macOS window corners could clip the echo line] → its text is 12px in and centred in 26px, with no bottom gap.
- [Terminal grids change size for existing users] → they resize on the next frame, as after any layout change.
- [Cascadia Code isn't installed on the Linux dev machine] → an automated test covers the font's features; the drawn `->` is a hand check.
