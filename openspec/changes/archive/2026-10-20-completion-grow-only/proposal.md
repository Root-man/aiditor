# Proposal

## Why

The completion list shrinks and grows with the number of matches, so the windows above it jump while you type or move between directories in `find-file` (#160).

## What Changes

- While one minibuffer is open, the completion list keeps the most rows it has shown, padding with empty rows (Emacs's `resize-mini-windows` = `grow-only`, Vertico's default).
- Closing the minibuffer resets it: the next one starts at the height its own candidates need.

## Capabilities

### New Capabilities

### Modified Capabilities
- `minibuffer`: the frame display's completion list only grows while a minibuffer is open.

## Impact

`crates/aiditor-core/src/minibuffer.rs` (the per-prompt completion state and the minibuffer display model); `docs/editor.md` minibuffer paragraph. The frontend draws the padded rows as it draws any row; no change there.
