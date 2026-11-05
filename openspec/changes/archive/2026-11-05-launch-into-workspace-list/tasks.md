# Tasks

## 1. Optional current workspace

- [x] 1.1 Make `Buffers.workspace` and `Core::workspace()` optional, and have `current_workspace()` return `Result<_, String>` with `No workspace`. Update every caller: frame, modeline, visiting, workspace list, project commands, grep, terminal, agents, sessions, jump and environment. `Core::default()` still makes its launch workspace at this step; verify `cargo test --workspace` passes unchanged.
- [x] 1.2 Make buffers created with no current workspace global. `show_workspace` and `new_workspace` accept a `None` current, dropping the outgoing lobby windows. Verify with a core unit test that makes a workspace from a `None` current.

## 2. Lobby state

- [x] 2.1 Add `show_lobby`: one window on a fresh `*Workspaces*` owned by no workspace, kept alive while in the lobby. `new_workspace` kills the lobby list when leaving the lobby. Verify with harness tests "Leaving the lobby" and "Messages survives the lobby".
- [x] 2.2 Drop the sole-workspace error from `kill_workspace` and show the lobby after the last kill, keeping the save and process questions. Update the existing sole-workspace tests to the new scenarios and verify the save-decline scenario keeps the workspace.
- [x] 2.3 In the lobby, `kill_buffer` with no replacement shows `*Messages*`, made if need be, instead of making a scratch. Verify with a test that kills the lobby list and reopens it with `C-x t l`.

## 3. Launch

- [x] 3.1 `Core::default()` starts in the lobby with no `$HOME` project. `Core::launch` makes the launch workspace for a path instead of rebinding one. `Harness::new` makes the `main` workspace explicitly, and a lobby harness constructor is added. Verify `workspace.rs` launch tests, `boundaries.rs` and the editor tests pass, and a new test shows "No argument" lists only saved projects.
- [x] 3.2 Move `root()` from `main.rs` to `aiditor_services::files` and use it from `main.rs`. Verify the existing `main.rs` launch tests still pass.

## 4. Lobby list

- [x] 4.1 With no current workspace, put initial point on the first project row and render `No projects` for an empty snapshot, with `0 projects, 0 workspaces` and no position. Verify with snapshot tests for both cases.
- [x] 4.2 Make `q` and `C-g` keep the lobby list, and make `RET` on a project row make its workspace. Verify the "RET on a project" and "Quit keeps the lobby" scenarios.
- [x] 4.3 Add the lobby echo hints (`… C-x C-f find file   C-x p p project`) to the hint table. Verify with a harness test, and with a GPUI test that the lobby hints wrap and are styled like the list hints.

## 5. Leaving the lobby by command

- [x] 5.1 Make `VisitIntent.workspace` optional and add `root` to `Msg::Opened`, filled by the file service for lobby visits. In the lobby, `directory()` falls back to `$HOME`. Verify a services test that a lobby open reply carries the `.git` root.
- [x] 5.2 `on_opened` in the lobby resolves the project, makes its workspace and visits the file; an error only echoes. Verify the scenarios "find-file in a git repo", "find-file in a saved project", "find-file starts at home" and "A failed visit stays in the lobby".
- [x] 5.3 `Msg::Recover` entries carry each file's root from the recovery service's listing, and opening one in the lobby makes its workspace. Verify with an autosave test from the lobby harness.
- [x] 5.4 `workspace-new` in the lobby selects the first project initially, and `project-switch` works from the lobby. Verify with harness tests.
- [x] 5.5 Verify that `project-find-file`, `project-grep`, `vterm`, `agent-start`, `agent-sessions` and the workspace switch, cycle, rename and kill commands echo `No workspace` in the lobby, with a table-driven harness test.

## 6. Documentation and integration

- [x] 6.1 Update `docs/editor.md`: Projects ("Launching without a project", superseding the #189 no-argument rule) and Workspaces (the lobby, killing the last workspace). Verify the text matches the spec delta.
- [x] 6.2 Run `cargo fmt --check`, `cargo clippy --workspace --all-targets` and `cargo test --workspace` on macOS. Launch the app with no argument and check the lobby list, `C-x C-f` from it, and killing the last workspace. Record anything not checked on Linux.
  - Checked on macOS by hand; not checked on Linux.
