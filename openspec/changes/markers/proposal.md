# Proposal

## Why

Point and mark are plain offsets today, so an edit made elsewhere in the buffer (another window, an agent, a revert) leaves them pointing at the wrong text. Buffers need markers, positions that follow edits, before the window model can keep a point and mark per window (issue #15).

## What Changes

- A buffer holds markers: positions that move with inserts and deletes made through the buffer's edit API.
- Each marker has an insertion type: it stays before text inserted at its position, or advances past it.
- Markers can be added, read, moved and removed; refused edits leave them unchanged.
- `docs/editor.md` records that a window's point follows edits as a marker in its buffer.
- Not in this change: turning the core's point into a marker (comes with the window model).

## Capabilities

### New Capabilities

### Modified Capabilities
- `text-buffers`: buffers gain markers that follow edits.

## Impact

`crates/aiditor-core/src/buffer.rs` (new marker API, edits update markers) and `docs/editor.md`. No new dependencies.
