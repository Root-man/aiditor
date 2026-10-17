# Tasks

## 1. Buffer history

- [x] 1.1 Record inserts and deletes into undo groups in `Buffer`, with `undo_boundary(point)`, `undo()` and `redo()` returning the point to restore; verify with unit tests for grouping, round trips, redo clearing and refused edits recording nothing

## 2. Keys

- [x] 2.1 In `Ctx::key`, set a boundary per key with up to 20 self-inserts amalgamated, bind `C-/`, `C-_`, `C-x u` to undo and `C-?` to redo, and report an empty stack in the message line; verify with harness tests for every scenario in `specs/undo/spec.md`
- [x] 2.2 Record the redo-stack model and the bindings in `docs/editor.md`; verify the Commands and keymaps section names them

## Workflow follow-up

- Verify, archive, open and merge the PR per the /implement skill.
