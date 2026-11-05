# Proposal

## Why

A launch without an argument makes a workspace on an ad-hoc `$HOME` project that nobody asked for, and the first thing done in it is usually leaving it for a real project. Opening the workspace list at launch, with no workspace yet, lets the first choice be the project or file actually wanted, and gives the welcome dashboard (#193) the same no-workspace starting point later.

## What Changes

- **BREAKING** A launch that names no file or folder makes no workspace and no `$HOME` project. It opens a global `*Workspaces*` list, the lobby, owned by no workspace, as the only window. This replaces the launch rule decided in #189.
- The editor can have zero workspaces. This lobby state starts the app and returns when the last workspace is killed.
- **BREAKING** Killing the last workspace no longer fails with `Attempt to delete the sole workspace`. After the usual save and process questions it kills the workspace and shows the lobby.
- In the lobby list:
  - `RET` on a project row makes or shows that project's workspace, as it does today.
  - `q` and `C-g` keep the list, since there is nothing to go back to.
  - The echo hints name `C-x C-f` and `C-x p p` instead of `q quit`.
  - With no projects, the list shows `No projects`.
- Some commands can leave the lobby:
  - `find-file` (`C-x C-f`) starts at `$HOME`. The visited file gets a workspace on its project, resolved the way a command-line file is.
  - `project-switch`, `workspace-new` and recovering a file make a workspace on that file's project.
- Commands that need a workspace fail in the lobby with `No workspace`: project find-file and grep, terminals, agents and the workspace commands other than `workspace-new` and `workspace-list`.
- Launching with a file or folder is unchanged.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `workspaces`: launch with no workspace, the lobby state, killing the last workspace, buffer ownership with no workspace, and the lobby list's keys, hints and empty state.

## Impact

- `aiditor-core`:
  - The current workspace becomes optional: `Buffers.workspace`, `Core::workspace()` and `current_workspace()`. This touches every module that reads it: frame and modeline, visiting, workspace list, project commands, grep, terminal, agents, sessions, jump and environment.
  - `Core::default()` and `launch`.
  - The visit intent and `Msg::Opened` carry an optional workspace and the visited file's project root.
- `aiditor-services`: the file service resolves a file's project root for lobby visits.
- `aiditor` app: `main.rs` launch and its `root()` helper, which moves to the services crate, plus GPUI tests that assume a launch workspace.
- Fixtures and the harness that start with `*scratch*<home>` or `*scratch*<main>`.
- `docs/editor.md`: Projects ("Launching without a project") and Workspaces.

No new dependencies and no persistence changes.
