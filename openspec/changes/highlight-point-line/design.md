# Design

## Context

`Decoration::Background` already paints a row's full width, gutter included, before the row's number and text, and the text's span backgrounds paint after it. `Core::frame` decides per window what a window shows, such as its gutter.

## Goals / Non-Goals

**Goals:** the behavior in `specs/line-highlight/spec.md`.

**Non-Goals:** a per-buffer toggle; drawing `match` faces, which no code applies yet; `hl-line-sticky-flag`.

## Decisions

**A decoration added in `Core::frame`, not a face on spans.** The row is filled past its text and under the gutter, which only a decoration does, and the region and matches stay on top with no new logic. `display::build` doesn't know its window is selected, and the minibuffer's own line calls `window_display` without going through `frame`, so it stays plain.

**The rows follow the cursor's display line, not point's buffer line.** A line joined by a fold starts on an earlier buffer line, so the cursor's `Line::number` finds it where `line_col(point)` would not, and all rows of a wrapped line share that number, as Emacs highlights a logical line. Decorations go first in the list so a buffer's own line backgrounds stay on top.

**The minibuffer's origin window counts as selected.** Frame 10 draws `hl-line` in swiper's previewed window. `windows.selected` is the minibuffer while one is open, so `frame` also compares against its `from`.

**`*scratch*` and `*Messages*` are included,** through `Buffer::numbered`, because they are drawn like files and the line numbers made the same call.

## Risks / Trade-offs

- [Frame snapshots of file buffers gain the decoration] → Updated in this change, and they show it.
- [Paint order is a frontend fact a core test can't see] → It is the same order session blocks rely on; the display test checks the model, and the review step checks the frame.
