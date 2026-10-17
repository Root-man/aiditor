# Design

## Context

`Window` holds point and an optional mark as buffer markers; `display::build` takes `mark: Option<usize>` and highlights whenever it is `Some`. Commands run through `Ctx::call`, both from keys and from the harness. `keyboard-quit` is a builtin that only sets the message.

## Goals / Non-Goals

**Goals:** Emacs's mark behavior for one window, testable through the harness and display snapshots.

**Non-Goals:** per-buffer marks shared across windows (there is one window), the global mark ring.

## Decisions

- **Ring and active flag live on `Window`**, beside the mark, as the data model puts the mark there. The ring is a `Vec<MarkerId>` of markers in the window's buffer, so ring entries follow edits like the mark does. Pushing moves the mark's marker id onto the ring front and adds a new marker; popping rotates ids without new markers; dropping past 16 removes the marker from the buffer. `show` removes them all, as it already does for point and mark. Alternative: positions as plain `usize`, which would go stale after edits.
- **The display gets the mark only while active**: `Core::display` passes `None` for an inactive mark, so `display::build` does not change.
- **Deactivation after a buffer change** uses a modification tick on `Buffer`, bumped by every raw insert and delete (undo and redo included). `Ctx::call` compares the current buffer's tick before and after the handler and deactivates on a change. Alternative: deactivate in each editing helper, which every future editing command would have to remember.
- **`M-<`/`M->` push the mark** in `buffer_end` before moving, skipped when the mark is active or the prefix is `Universal`.
- **Ctrl+Space**: GPUI reports the key as `space`, which the Ctrl/Alt branch of `to_keys` drops as a multi-char name; map `space` to `' '` there.

## Risks / Trade-offs

- [macOS binds Ctrl+Space to switching input sources by default, so `C-SPC` may never reach the editor] → note it in the docs; the user can free the shortcut or rebind `set-mark-command` in `keymap.toml`.
