# Proposal

## Why

Every edit is permanent today: a typo or a wrong `DEL` can only be fixed by retyping. The command loop and auto-revert need undo grouped per command, so the buffer must keep an edit history now.

## What Changes

- Each buffer keeps a linear undo history of edit groups and a redo stack.
- Each key command is one undo group; up to 20 consecutive typed characters amalgamate into one group, like Emacs.
- Undo reverts the last group and puts point back where it was before that command; redo re-applies it and puts point where it was after.
- A new edit after undo clears the redo stack.
- Keys: undo on `C-/`, `C-_` and `C-x u`; redo on `C-?`. With nothing to undo or redo, the message line says so.
- Out of scope: `s-z` (keys have no super modifier yet), `C-M-_`, amalgamating `DEL`, undo in region, clearing the modified flag when undo reaches the saved text, and auto-revert's undo group (its own issue).

## Capabilities

### New Capabilities
- `undo`: per-buffer undo and redo of edit groups, how groups are formed, and where point goes.

### Modified Capabilities

## Impact

- `crates/aiditor-core/src/buffer.rs`: history recorded by `insert`/`delete`, plus boundary, undo and redo.
- `crates/aiditor-core/src/lib.rs`: the hardcoded key handler sets a boundary per command, amalgamates self-inserts, and binds the undo and redo keys.
- `docs/editor.md`: records the redo-stack model and the bindings.
