# Proposal

## Why

The M0 gate's manual run on macOS ([#54](https://github.com/Root-man/aiditor/issues/54)) passed every check but turned up six notes: text touches the window edge, where the rounded corners clip the minibuffer's first letter; the cursor is a thin bar; the key hint reads `u undo`; a completing prompt shows no match count; the region never shows in `*Messages*`; and `*scratch*` is auto-saved and offered for recovery after a kill, which Emacs does not do.

## What Changes

- The frame gets an 8 px gap on the left and right (Emacs's fringe width) and 2 px at the bottom.
- The cursor is a block one char wide in the cursor color, the char under it drawn in the background color; at the end of a line it is one cell wide.
- Key hint entries read `key : command`, the keys right-aligned so the colons line up, as which-key.
- A completing minibuffer shows the selected match's number and the match count before its prompt, as vertico does: `1/100 M-x `, `0/0 ` with no match.
- Logging to `*Messages*` no longer counts as a change to it, so a command's own echo (`Mark set`) does not deactivate the mark there.
- Only buffers visiting a file are auto-saved, as Emacs's `auto-save-default`. A no-file recovery file left by an earlier session can still be recovered, and is deleted once it is.
- Out of scope, filed for M5: an Emacs-like `find-file` list (`*/N` until a file is picked, dirs colored, mode bits, size, age).

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `frontend-rendering`: block cursor, the `key : command` hint.
- `minibuffer`: the match count in the frame display.
- `auto-save`: file buffers only.
- `mark`: logging to `*Messages*` is not a change.

## Impact

- `crates/aiditor/src/editor.rs`, `element.rs`; `crates/aiditor-core/src/buffer.rs`, `autosave.rs`, `minibuffer.rs`, `display.rs` (`Minibuffer::count`); tests in those and in `aiditor-services/src/recovery.rs`.
- No new dependencies.
