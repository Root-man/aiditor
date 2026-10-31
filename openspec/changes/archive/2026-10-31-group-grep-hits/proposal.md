# Proposal

## Why

Project grep repeats the file path on every hit. Issue #351 calls for the approved file-group layout so nearby hits are easier to scan.

## What Changes

- Group hits once per file in first-seen file order, retaining each file's hit order. Later streamed hits join their file's existing group.
- Draw file names on separator rules and hits as line number plus text, using the existing grep faces and the user's frame 3 export.
- Move `[searching]` and `[no matches]` to the right of the echo row with fixed reserved space, so status transitions never shift prompt input or horizontal scrolling (requested during manual review).
- Keep headers outside selection and counts, and retain the selected hit and preview across batches.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `project-grep`: grouped display and streaming order replace the flat arrival-order list.

## Impact

Core grep ingestion, minibuffer viewport/display data, GPUI completion rendering, their tests and `docs/editor.md`. No service protocol or dependency changes. No changes to query syntax, status lifecycle, hit limits or match highlighting.
