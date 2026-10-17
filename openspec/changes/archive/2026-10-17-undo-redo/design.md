# Design

## Context

`Buffer::insert` and `Buffer::delete` edit the rope and move markers. There is no command loop yet: `Ctx::key` in `lib.rs` is a hardcoded key handler, and the window's point is a marker the buffer cannot tell apart from others. See proposal.md for why.

## Goals / Non-Goals

**Goals:** history in the buffer so every edit path records it; a boundary call the future command loop can make once per command.

**Non-Goals:** Emacs's undo-of-undo ring, undo in region, undo limits by size, restoring the modified flag.

## Decisions

- **History lives in `Buffer`, recorded by `insert`/`delete`.** Every edit path (keys, later auto-revert, agents) gets undo for free. The raw rope-and-marker edit becomes a private helper that undo and redo call, so they do not record themselves. Alternative: a history per window or in `Core`; rejected because undo is per buffer in the spec.
- **A group is pushed on its first edit and stays open until the next boundary.** `undo_boundary(point)` closes the open group, storing `point` as its after-point, and remembers `point` as the before-point of the next group. The buffer never sees windows, so the caller passes point. If no boundary set them, a group falls back to the edit's own position (start of the first change, end of the last), so direct API use still gives sane positions.
- **Changes store their text.** `Insert(at, text)` and `Delete(at, text)`; undo applies inverses in reverse order, redo applies forward. A deleted range is read with `get_slice` before removal, so a refused delete records nothing.
- **Linear undo plus redo stack**, not Emacs's undo-of-undo: what the spec and issue ask for, and simpler to reason about. Any recorded edit clears the redo stack.
- **Amalgamation is the caller's job.** `Core` counts consecutive self-inserts; `Ctx::key` skips the boundary while the count is 1..20, so up to 20 typed chars share one group. The command loop takes this over later.
- **`C-?` ignores shift.** GPUI on Linux reports Ctrl+Shift+/ as `?` with shift held; macOS drops shift for non-letters. Comparing with shift cleared matches both. Same for `C-_`.

## Risks / Trade-offs

- [Unbounded history] → fine for the MVP's buffer sizes; `shortcut:` comment, cap it when long sessions show memory growth.
- [Undo to the saved text leaves the buffer modified] → acceptable until the modified flag tracks a save point in history.
