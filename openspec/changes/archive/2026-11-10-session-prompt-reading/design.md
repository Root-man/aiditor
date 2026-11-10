# Design

## Context

`session_display` (frame.rs) pins the transcript to its end whenever point is in the prompt area, by computing the top from the prompt line each frame. Point is one marker on the `Window`; `scroll_lines` and `recenter` (motion.rs) always move the view around point. `Ctx::call` already has a post-command fix-up for sessions (point off the `> ` marker).

## Goals / Non-Goals

**Goals:** the transcript view independent of point in the prompt area; `C-t` between two remembered points; typing from the transcript reaches the prompt.

**Non-Goals:** mouse scrolling; keeping the prompt pane's own top (still the shortcut from #441); several windows on one session sharing a reading spot (each window keeps its own).

## Decisions

- **Follow flag on the window.** `Window::follow` is true when the last redisplay's transcript showed its end (its top at or below the top that ends at the prompt line), and `set_top` clears it so paging counts without a redisplay. With point in the prompt area, a following window pins to the end; otherwise it keeps its stored top, clamped so it never scrolls past the end, and writes the clamp back. A new window follows; a split copies the flag with the top. Alternative: a flag on the session; rejected, as each window has its own view.
- **Two remembered points as markers on the window.** `Window::parts`, the transcript point and the prompt point, created lazily and released with the window's other markers. `Ctx::call`'s post-command hook records point into the one for the part point is in, so leaving either part any way updates it.
- **Scrolling from the prompt area** works on the window's top in lines, clamped to `[0, end top]`, and then moves the reading point into the rows shown, like Emacs's scroll moves point. At either end it echoes `Beginning of buffer` / `End of buffer` as `scroll_lines` does. `C-l` recenters the reading point's line (middle, top, bottom cycle), as if point were there; without a reading point it recenters the transcript's last line.
- **`agent-toggle-prompt`.** From the prompt: puts the reading point within the shown rows (start of the top row if above, the last shown line's start if below or unset) and goes there; `scroll` then finds point visible and keeps the rows. From the transcript: goes to the drafting point, else the buffer's end; the rows stay since the window keeps its top.
- **Typing redirect in `self-insert-command`.** In a session with point before the prompt start, point first moves to the drafting point (else the end). This covers unbound chars and the bound keys that fall back to self-insert (`/`, digits and `o` off a question), which matches "typing goes to the prompt"; keys bound to other commands are untouched.
- **Sending shows the end.** `agent-send-input` sets `follow`, so the reply is in view.

## Risks / Trade-offs

- Row clamping on wrapped lines walks lines, not rows, so after a page the reading point lands on a line start; acceptable.
- `M->` from far up in the transcript now keeps the old rows; `C-v` or sending returns to the end.
