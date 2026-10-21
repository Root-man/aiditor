# Tasks

## 1. Saved configurations and switching

- [x] 1.1 Add `Windows` (selected window, its id, the others, the tree) and `Workspace::windows: Option<Windows>`. Add `Core::switch_workspace(id)`: it closes the minibuffer, swaps `Core`'s window fields with the target's saved ones, stores the old ones on the old workspace and raises the shown buffer. Verify with harness tests: switching away and back restores splits, buffers, points and the selected window; a split deleted in one workspace leaves the other's alone; points follow edits made while away; a switch with a prompt open closes it; switching to the current workspace changes nothing.
- [x] 1.2 Make `new_workspace` install a fresh single window on its scratch, saving the old configuration, and make `kill_workspace` switch to the neighbour first and release the killed workspace's saved windows. Verify with tests: the previous workspace keeps its split after a new one is made; killing the current workspace shows the neighbour's split; existing workspace tests still pass.
- [x] 1.3 Make `Core::kill_buffer` replace the killed buffer in every saved configuration with that workspace's most recent listed buffer or `*Messages*`. Verify with a test: a window of an inactive workspace that showed another workspace's buffer shows its own workspace's buffer after that buffer is killed, with no panic on switching back.
- [x] 1.4 In `docs/editor.md` under Workspaces, add a "Built in #63" note and drop the shared-tree item from #62's "Not yet". Verify the note is present.

## Workflow follow-up

- Archive the change after review.
