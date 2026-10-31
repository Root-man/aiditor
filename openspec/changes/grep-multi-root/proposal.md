# Proposal

## Why

In large multi-root projects, unrelated roots can fill the grep list before the root being edited. Issue #340 makes local hits arrive first and lets a prefix restrict the same command to one root.

## What Changes

- Search the current file's root first, then the remaining roots in project order; buffers outside the roots use project order.
- With a prefix argument, complete over the project's root names and search only the selected root.
- Preserve streaming within a root, parallel file workers, cancellation and the global hit cap.
- Update `docs/editor.md` and regression tests.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `project-grep`: ordered root traversal and prefix-triggered single-root selection.

## Impact

Core grep command/prompt state, the grep service and its tests, and the editor specification. No dependencies or new commands.
