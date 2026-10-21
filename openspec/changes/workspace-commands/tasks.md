# Tasks

## 1. Required project and launch

- [x] 1.1 Make `Workspace.project` a `String`, bind the launch workspace to an ad-hoc `$HOME` project, track `shown`, and keep ad-hoc projects on `ProjectsLoaded`; update the workspace and project tests and verify `cargo test -p aiditor-core` passes
- [x] 1.2 Add `project_for(path, root)` and `Core::launch(path, root)`; verify with tests for a saved project's root, an existing ad-hoc primary, a new ad-hoc name with `<2>`, and the `$HOME` project dropped
- [x] 1.3 In `main.rs`, pass the git repo or folder as `root` and open a directory argument as a folder; verify with a test of the root lookup

## 2. Commands

- [x] 2.1 Add `workspace-new`, `workspace-switch`, `workspace-next`, `workspace-previous`, `workspace-rename`, `workspace-list` and their `C-x t` keys; verify with key-script tests for each spec scenario
- [x] 2.2 Share the `C-x C-c` questions and add `workspace-kill`; verify the quitting tests still pass and key-script tests cover nothing modified, save then kill, decline, kill anyway, other workspaces' buffers and the sole workspace
- [x] 2.3 Add `workspace-open-folder` and `project-switch` (`C-x p p`); verify key-script tests for a new folder writing no file, reopening it, a folder inside a saved project, and switch project reusing the most recent workspace or making one
- [x] 2.4 Verify the `C-x t` and `C-x p` hints list every command

## 3. Docs

- [x] 3.1 Add "Built in #64" notes under Workspaces and Projects in `docs/editor.md` and drop the `default` workspace wording; verify the text matches the specs
