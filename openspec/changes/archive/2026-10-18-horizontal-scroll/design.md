# Design

## Context

`display::build` emits lines whole and the frontend clips them at the window's content mask. The core knows each window's pixel rect and the cell width (`Msg::Resize`), so it can work out the window's width in columns. Vertical scrolling is a clamp of `Window::top` at redisplay.

## Goals / Non-Goals

**Goals:** keep the cursor visible on long lines; leave other lines alone.

**Non-Goals:** soft wrap (#112), `hscroll-margin`/`hscroll-step`, `scroll-left`/`scroll-right` commands, truncation glyphs, tab and wide-char widths.

## Decisions

- **The offset lives in `Window`, applied at redisplay**, like `top`: `Window::hscroll(col, cols)` returns 0 when `col < cols`, else clamps the old offset to `col-(cols-1)..=col` and stores it. Stateless (always minimal from 0) would also work for `C-e`, but would jump the line on every move left; keeping state matches vertical scrolling.
- **The core cuts point's line**, not the frontend: `window_display` takes the column count, builds the display, then drops the offset's chars from the cursor's line and shifts the cursor column. The display snapshot then shows exactly what is drawn, and the frontend needs no change for drawing.
- **Columns** are `rect.width / cell_width`, floored; the minibuffer gets the frame's columns less the prompt's chars. `Core::display(height)` has no width and passes `usize::MAX`, so it never scrolls.
- **Clicks**: `click(line, col)` adds the selected window's offset when `line` is point's line, since that is the line drawn scrolled.

## Risks / Trade-offs

- [A tab or wide char counts as one column, so the cursor can still sit past the edge on such lines] → `shortcut:` comment; fix when the display measures widths.
- [A fold on point's line: the offset counts shown chars while a click maps shown columns as buffer columns] → the existing click shortcut covers it.
