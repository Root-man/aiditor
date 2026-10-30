# Design

## Context

The core lays windows out in `Core::frame`, which knows each rectangle and the cell width and passes `cols` to `window_display`; wrapping, `C-n` by rows and `hscroll` all work from that `cols`. The frontend shapes each `Line` and paints it at the window's left edge; the cursor's x comes from the shaped line. Design: frames 1, 2 and 11, with `docs/editor.md` settling the gutter at 10px + digits + 12px at every width.

## Goals / Non-Goals

**Goals:** the gutter of frames 1 and 11, in file buffers, `*scratch*` and `*Messages*`.

**Non-Goals:** `hl-line` (#354) and the frame's 8px side inset (#353), so the numbers stand 18px from the frame edge until #353. Numbers in sessions, terminals, grep and other generated buffers, which the frames draw without one. A binding for the toggle, a minimum digit count, Emacs's `visual` style. Persisting a toggle across restarts.

## Decisions

- **The gutter lives on `FrameWindow`, not `WindowDisplay`.** It is laid out in pixels, which only `frame()` has, and the window display stays in char columns. Tests of `Core::display` keep their snapshots. Alternative: on each `Line`, which touches every snapshot and carries data most buffers don't use.
- **`frame()` subtracts the gutter before computing `cols`.** Everything downstream (wrap, row motion, hscroll) then needs no change, as `Window` keeps the reduced `cols` from `set_cols`. The minibuffer and terminal paths are untouched: neither visits a file.
- **Continuation rows are found from the display lines.** Wrapped rows share `Line::number`, so a row whose number equals the previous row's is a continuation, and the first row is one when `window.top_row() > 0`. Alternative: a row index on `Line`, more state for the same answer. Fold-joined lines don't collide: each display line starts on a later buffer line.
- **The core sends text and faces; the frontend aligns.** `Gutter { width, rows }` holds the full width in px, and the frontend places each shaped label so it ends `GUTTER_RIGHT` before the text. `↪` may come from a fallback font, so padding with spaces would not align it.
- **A per-buffer `Option<bool>` over the setting.** `None` follows the setting, so `C-x C-r` reaches untoggled buffers; a resolved style stored on the buffer wouldn't. `Some(true)` over `off` means absolute.
- **`*scratch*` and `*Messages*` are recognised as the unkillable buffers.** They are the only two, and a third flag on `Buffer` would say the same. Sessions stay out: their block backgrounds and rules start at the window's left edge.
- **The digits count the buffer's lines, not the shown ones**, so the text doesn't shift while scrolling. Relative style uses the same width, since point's line shows its absolute number.
- **Harness keeps `line_numbers` off.** Dozens of frame tests assert columns of file buffers; they test wrapping, not the gutter. Gutter tests turn it on.

## Risks / Trade-offs

- [The gutter narrows every file window, and the width changes at 10, 100, 1000 lines] → That is the issue's behaviour; `cols` follows at the next redisplay.
- [Wrapped-row scrolling reads `cols` from the last redisplay, so the frame after a width change scrolls with the old width] → Already so for resizes; the next `frame()` corrects it.
