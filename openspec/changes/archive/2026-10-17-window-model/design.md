# Design

## Context

`Core` holds `current: BufferId` and `point: usize`; the frontend holds `top` and clamps it in `render`. Buffers already have markers with insertion types (#15).

## Goals / Non-Goals

**Goals:** point and mark as markers, the scroll position in the core, tested through the core.

**Non-Goals:** reporting frame size to the core (display-model issue), mark commands and the region (mark-region issue), goal column (motion issue), splits.

## Decisions

- **`Window { buffer, point, mark: Option<MarkerId>, top }` in `aiditor-core/src/window.rs`.** Methods take `&Buffers`/`&mut Buffers`, so the window holds only a buffer id (spec: a buffer never knows its windows) and can never read a marker from the wrong buffer.
- **Point and mark are `Stay` markers**, like Emacs's window-point: commands that type at point set point explicitly, so another window's point at the same spot stays before the inserted text.
- **Scrolling at redisplay.** `Core::scroll(rows) -> top` clamps `top` into `[point_line - (rows - 1), point_line]` and returns it; the frontend calls it in `render` with the rows it computed. Alternative: a resize message storing rows in the core; that belongs to the display-model issue, which reports frame size anyway.
- **Showing another buffer** (`Core::open`) removes the window's markers from the old buffer and adds new ones at 0 in the new one; `top` resets to 0.

## Risks / Trade-offs

- [Scroll is only updated at render] → fine while only the frontend reads it; scroll commands will need rows in the core and get them from the display-model issue.
