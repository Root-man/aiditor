# Tasks

## 1. Model

- [x] 1.1 Add `workspace.rs` with `WorkspaceId` and `Workspace { id, name, project }`; give `Buffer` a `workspace` field stamped from a current workspace held by `Buffers`; `*Messages*` gets `None`; `Core` starts with the `default` workspace. Verify with a test that the launch scratch belongs to `default` and `*Messages*` to none.
- [x] 1.2 Scope `Buffers::visiting` to the current workspace and filter `Core::kill_buffer`'s replacement to the current workspace or `*Messages*`. Verify with tests: a file visited in two workspaces gives two buffers; killing a new workspace's scratch does not show the other workspace's file.

## 2. Lifecycle

- [x] 2.1 Add `Core::new_workspace(name, project)` (current, new scratch in the selected window) and `Core::kill_workspace(id)` (kills owned buffers, selects the neighbour when current, refuses the last). Verify with lifecycle tests: killing a workspace removes its buffers and leaves other workspaces, their buffers and `*Messages*` untouched; killing the current one selects its neighbour; killing the sole one fails.
- [x] 2.2 Record "Built in #62" under Workspaces in `docs/editor.md`, including the shared window tree and global names as known limits. Verify the note is present.

## Workflow follow-up

- Archive the change after review.
