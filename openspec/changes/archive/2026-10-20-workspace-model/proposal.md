# Proposal

## Why

Workspaces are the unit of work in aiditor (docs/editor.md, Workspaces), but the core has one global buffer list. The workspace commands (#64), per-workspace windows (#63) and scoped switching (#66) all need a model in which every buffer has an owning workspace. This change (#62) adds that model.

## What Changes

- Workspaces in the core: each has a name and the name of the project it is bound to. The core keeps them in creation order and has one current workspace. At launch there is one workspace, `default`, bound to no project until the workspace commands (#64) bind one.
- Every buffer belongs to the workspace that was current when it was made. `*Messages*` belongs to none (decided in #61).
- Visiting a file reuses a buffer only from the current workspace; a file open in another workspace gets a second buffer.
- A new workspace becomes current and starts with one empty scratch buffer, shown in the selected window.
- Killing a workspace kills every buffer it owns and leaves other workspaces' buffers and `*Messages*` alone. Killing the current one makes its neighbour current. Killing the last one is refused.
- Killing a buffer replaces it in windows with a buffer from the current workspace or `*Messages*`, never another workspace's buffer.

No commands or keys are added; #64 adds them, and also the prompt about modified buffers before a workspace is killed.

## Capabilities

### New Capabilities
- `workspaces`: workspaces, their project binding, buffer ownership and their lifecycle (create, kill).

### Modified Capabilities
- `file-visiting`: a file already visited is reused only within the current workspace.
- `buffer-switching`: the buffer that replaces a killed one comes from the current workspace or is `*Messages*`.

## Impact

`aiditor-core` only: a new `workspace` module, a workspace field on `Buffer`, the current workspace in `Buffers`, and `Core`'s visiting and kill-buffer paths. No frontend or services change. `docs/editor.md` gets a "Built in #62" note under Workspaces.
