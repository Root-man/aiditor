## Context

`Ctx::next_line` moves `n` buffer lines to `goal_column`, a char column set when a run of `next-line`/`previous-line` starts. `Window::scroll` already computes a buffer line's row starts from the window's `cols` (set at the last redisplay) with `Buffer::wrap` and `display::row_starts`, in buffer chars.

## Goals / Non-Goals

**Goals:** row motion that agrees with the rows `scroll` and the display use.

**Non-Goals:** `C-v`, `M-v`, `C-l` and jumps by rows; display-width goal columns (tabs, wide chars, folds); Emacs's `track-eol`; `visual-line-mode`'s `C-a`/`C-e`/`C-k` rebinds.

## Decisions

- **Share the row starts.** `scroll`'s `starts` closure becomes `Window::row_starts(&self, buffer, line) -> Vec<usize>` (`[0]` when the buffer truncates), used by `scroll` and `next_line`. So motion and scrolling can't disagree on where a row breaks.
- **Goal relative to the row.** At the start of a run the goal is point's column less its row's start; `goal_column` keeps that meaning in a wrapping buffer, and the plain column otherwise (a row start of 0).
- **Step row by row.** From point's (line, row), walk `n` rows across line boundaries, fetching a line's starts only when entering it; past the first or last row it goes to the buffer's end and reports, as `goto_clamped` does now. Target column: row start + goal, clamped to the row's last char (`next start - 1`) on a non-last row and to the line's length on its last row. A count of `n` costs `n` steps, as Emacs.
- **Width from the last redisplay.** Before the first redisplay `cols` is 0, so `Buffer::wrap` returns `None` and motion is by lines, which matches what a window that never drew can show.

## Risks / Trade-offs

- [Rows are in buffer chars] → a fold on a wrapped line makes motion count rows the display doesn't show, the same shortcut `scroll` has.
- [A word-wrapped line is scanned whole for its starts] → a long Markdown paragraph costs its length per `C-n`; code lines break arithmetically.
