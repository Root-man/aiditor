# Proposal

## Why

Each workspace should keep its own layout (docs/editor.md, Workspaces and Core data model: a workspace owns its window configuration). Since #62, though, one window tree is shared by every workspace: a new workspace shows its scratch only in the selected window, and the old workspace's splits stay on screen. This change (#63) gives each workspace its own split tree.

## What Changes

- Each workspace has its own window configuration: the split tree, its windows (buffer, point, mark, scroll), and the selected window.
- A new core call switches to a workspace. It saves the current workspace's configuration and restores the target's, so switching away and back restores the splits, buffers and points.
- A new workspace starts with a single window on its `*scratch*`. The workspace that was current keeps its layout.
- Killing a workspace drops its windows. Killing the current one shows the neighbour's saved layout.
- Killing a buffer replaces it in every workspace's windows. In each workspace, the replacement is that workspace's most recent buffer or `*Messages*`.
- Switching closes an open minibuffer first.

No commands or keys are added: #64 adds switch, next/previous and the rest on top of the new call.

## Capabilities

### New Capabilities

### Modified Capabilities
- `workspaces`: adds a per-workspace window configuration and switching that restores it. A new workspace starts with a single window. Killing a workspace drops its windows, and killing a buffer replaces it in every workspace's windows.

## Impact

`aiditor-core` only: `workspace.rs` (a saved configuration on `Workspace`, `switch_workspace`, `new_workspace`, `kill_workspace`) and `Core::kill_buffer` in `visiting.rs`. No frontend or services change. In `docs/editor.md`, a "Built in #63" note goes under Workspaces, and the #62 note's "Not yet" loses its shared-tree item.
