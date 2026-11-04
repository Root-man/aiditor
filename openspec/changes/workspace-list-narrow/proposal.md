# Proposal

## Why

Workspace-list rows currently overflow split windows, hiding names and state. Issue #426 completes frame 13c's presentation while preserving the existing snapshot and navigation.

## What Changes

- Fit rows separately for each window: remove non-project where columns first, use state glyphs, shorten names at their end and project roots at their start.
- Drop mode-line inventory counts before shortening the list name, using shared mode-line fitting afterwards.
- Refit on resize without refreshing inventory or changing folds, point or decoration.
- Document the rules and automate core and frontend resize acceptance.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `workspaces`: narrow-window workspace-list presentation.

## Impact

Core workspace-list display and frame mode-line fitting, core/frontend tests, and docs/editor.md. No new dependencies, persistence or commands. Builds on the active project-rows, columns, folds and hints deltas.
