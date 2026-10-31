# Proposal

## Why

The workspace modeline names only the current workspace, hiding activity in other tasks. Issue #360 calls for the approved overview of all workspaces, their agent activity, and the local time.

## What Changes

- Show workspaces in creation order as `project › workspace`, separated by ` │ `; highlight the current workspace and dim the others.
- Aggregate agent activity per workspace as a dot, prioritizing waiting, failed, then working; idle sessions have no dot.
- Keep the current workspace and adjacent entries when space is limited, folding all other entries into `+N` and their most urgent activity dot.
- Draw a local 24-hour `HH:MM` clock at the right inset and refresh on minute boundaries.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `display-model`: replace the current-only workspace names with a fitted workspace modeline and activity faces.
- `frontend-rendering`: render the workspace spans and a right-aligned clock with minute updates.

## Impact

Core frame/display and agent status, GPUI editor rendering and clock wakeup, existing display snapshots and frontend tests, and the workspace display documentation. Reuse installed dependencies and existing faces; no workspace persistence or mouse switching.
