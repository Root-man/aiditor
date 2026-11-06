# Design

## Context

A `Window` holds one buffer id and its point, mark and scroll; `Window::show` rebuilds it for another buffer. `kill_buffer` picks one replacement per workspace from the global recent order (`replacement`) and `Windows::replace_buffer` shows it in every window that showed the killed buffer, in the current windows and every saved workspace's. Some ephemeral buffers (minibuffer, previews, the lobby's list) are killed with `Buffers::kill` directly, skipping that.

## Goals / Non-Goals

**Goals:** per-window history that survives workspace switches, used by the two commands and by buffer kills.

**Non-Goals:** a prefix argument for the commands; a point per history entry (Emacs keeps one); `C-x C-<left>`/`C-x C-<right>`; a full-frame workspace list.

## Decisions

- **History lives on `Window`** as two `Vec<BufferId>`, `prev` and `next`, most recent last. Saved workspaces keep their windows, so they keep their history for free. Alternative: a map in `Core` keyed by `WindowId` — needs upkeep on split, delete and workspace kill.
- **`show` records, a new private `display` doesn't.** `show` (every ordinary switch) pushes the old buffer onto `prev`, removes the new one from it and clears `next`; then calls `display`, which is today's `show` carrying the history across the rebuild. The commands and kills call `display` so they manage the lists themselves.
- **Buffer ids only, no point.** A window reopening a buffer starts at the buffer's remembered point, as today. Emacs keeps a marker per entry; that would add marker upkeep for little gain.
- **Dead ids are skipped where history is read**, besides being removed in `kill_buffer`, because some buffers are killed without `kill_buffer`. Showing a dead buffer would panic.
- **Fallback when a list is empty:** `previous-buffer` takes the most recent listed workspace buffer (as `C-x b` lists them) not shown and not in `next`; `next-buffer` the least recent not in `prev`. Excluding the other list makes repeated presses cycle through every buffer rather than toggle between two. The commands don't raise buffers, so the order they cycle in stays put.
- **`split` copies the history**, so `C-x <left>` works in either half after `C-x 2`.

## Risks / Trade-offs

- [Two windows on one buffer: the reopened buffer starts at the point of whichever window last stopped showing it, not this window's] → same as today's `C-x b`; a per-entry point can come later.
- [History grows with every switch] → each list holds a buffer once, so it is bounded by the number of buffers.
