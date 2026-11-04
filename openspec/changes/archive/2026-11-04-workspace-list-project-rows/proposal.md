# Proposal

## Why

The workspace list hides projects without workspaces and makes project headings inert. Issue #422 makes it a complete project entry point, following design frames 13–13d.

## What Changes

- List every project with its primary root; append projects without workspaces in name order with `no workspaces`.
- Navigate project and workspace rows, skipping informational buffer rows.
- Activate projects through existing project-switch behavior and retain workspace activation and dismissal.
- Show project/workspace totals and selectable-row position in the mode line.
- Update the editor documentation and capture the workspaces specification changes as a delta until archive.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `workspaces`: complete project inventory, selectable project rows and list mode line.

## Impact

Core workspace-list row metadata, tree generation, activation and mode-line rendering; their harness/frame tests and `docs/editor.md`. No new dependencies or persistence changes. Folding, styling, live refresh and destructive actions belong to other tasks.
