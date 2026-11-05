# Design

## Context

See proposal.md for the motivation. The core assumes a current workspace:

- `Core::default()` makes the `$HOME` ad-hoc project and a workspace with its scratch buffer.
- `Buffers.workspace` is a plain `WorkspaceId`. It names the current workspace, which owns every new buffer.
- `current_workspace()` and `workspace()` are read at about 125 call sites, many with `expect("current is live")`. They are in frame and modeline, visiting, workspace list, project commands, grep, terminal, agents, sessions, jump and environment.
- `show_workspace` saves the outgoing `Windows` into the current workspace.
- `kill_buffer` falls back to a new scratch in the current workspace.
- `find-file` starts in `directory()`, which falls back to the process working directory, `/` from the Dock.
- A file visit is asynchronous. `VisitIntent { workspace, window }` goes out with `FileOp::Open`, and `on_opened` drops the reply if the workspace or window changed.
- The core does no filesystem I/O. `main.rs` resolves a file's ad-hoc root, the nearest `.git` ancestor, with `root()` before calling `Core::launch`.
- `Harness::new` builds on `Core::default()` and renames the launch workspace to `main`.

## Goals / Non-Goals

**Goals:**
- Make "no workspace" a state the type system enforces. The compiler should find every site that assumed one.
- Leave the lobby only by making a workspace, so the lobby never owns user buffers.

**Non-Goals:**
- A welcome dashboard (#193, frame 8). The lobby is the plain workspace list.
- New visual design for the lobby beyond the `No projects` line and the hint text. They use existing faces.
- Restoring workspaces across launches.

## Decisions

### Lobby = `workspaces.is_empty()`, current workspace is `Option`

`Buffers.workspace` becomes `Option<WorkspaceId>`, and `Core::workspace()` returns `Option<WorkspaceId>`. `current_workspace()` returns `Result<&Workspace, String>`, with `Err("No workspace")`, so command handlers fail with `?`. Read-only callers such as frame and modeline match on `None`. The current workspace is `None` exactly when no workspace exists, so the lobby needs no separate flag.

Alternatives considered:
- A reserved `WorkspaceId` sentinel never listed in `workspaces`. It keeps signatures but hides absence behind the same `expect`s that would then panic.
- A hidden provisional `$HOME` workspace (option B). It keeps hidden state, and it receives buffers made before the first choice.

### Buffers made in the lobby are global

With no current workspace, a new buffer's `workspace` is `None`, like `*Messages*`. Only lobby-safe commands run there: the list, messages, help and the recovery prompt. So only `*Workspaces*` and `*Messages*` can be made. Leaving the lobby kills its global `*Workspaces*`. `show_workspace` does this when the previous current workspace was `None`, which covers `RET`, `project-switch`, `workspace-new` and lobby visits in one place. The list stays killable, since marking it unkillable would also turn on line numbers. `kill_buffer` with no current workspace and no replacement shows `*Messages*`, made if need be, instead of making a scratch; `C-x t l` brings the list back.

### Window configuration in and out of the lobby

`show_workspace` drops the lobby's `Windows` (releasing their markers) instead of saving them, since nothing owns them. Killing the last workspace calls a new `show_lobby`:
1. Release the windows.
2. Set `Buffers.workspace = None`.
3. Install a single window on a fresh lobby list.

`kill_workspace` loses its sole-workspace error. When killing the current workspace with no other left, it calls `show_lobby` where it used to switch to a neighbour.

### Launch

`Core::default()` makes no project, no workspace and no scratch. It shows the lobby list, so `focused` and the window always name a live buffer. `Core::launch(path, root)` changes from "rebind the sole workspace" to "make the launch workspace for `path`". It calls `project_for` and then `new_workspace` named after the project, and is valid only in the lobby. The `$HOME`-removal logic goes away. `Harness::new` calls `new_workspace("main", …)` on a test project, so the hundreds of tests that expect `*scratch*<main>` are unchanged. A separate harness constructor starts in the lobby for the new tests.

### find-file from the lobby: the file service supplies the root

In the lobby:
- `directory()` falls back to `$HOME`.
- `VisitIntent.workspace` becomes `Option<WorkspaceId>`.
- `FileOp::Open` is unchanged. `Msg::Opened` gains `root: Option<PathBuf>`. The file service fills it with the moved `root()` helper, now `aiditor_services::files::root`, also used by `main.rs`, when the intent has no workspace.
- `on_opened` with a `None` intent that is still in the lobby runs `project_for(path, root)`, then `new_workspace`, then the ordinary `visit`. An error reply only echoes it, so no project is made.

This keeps the core free of filesystem I/O. The alternative was a `.git` walk inside the core: a few `stat`s, but it would be the core's first filesystem access.

### Recovery in the lobby

`main.rs` already lists recovery files synchronously at launch, through `recovery::list`. That listing fills a new `Recovered.root` with the moved `files::root`. Opening one goes through the same `project_for` → `new_workspace` → `visit` path.

### Commands

`current_workspace()` returning `Result` gates handlers that read the workspace, but some of them prompt first. So `ArgSpec` gets a `workspace` flag, checked in `try_call` before any prompt. It is set on the commands the spec lists, which then fail with `No workspace` without asking anything.
- `project-switch`, `workspace-new` (with no initial project the first one is selected) and `workspace-list` work unchanged once `new_workspace` and `show_workspace` accept a `None` current.
- A fileless recovered buffer has no file to resolve a project from, so recovering one in the lobby makes a workspace on an ad-hoc `$HOME` project.

### Lobby list rendering

The snapshot builder already handles projects with no workspaces. With no current workspace, initial point goes to the first selectable row, and an empty snapshot renders `No projects` in `annotation`. Modeline counts come from the snapshot. The hint table gets a lobby variant, chosen when `workspace()` is `None`. `workspace_list_quit` returns `Ok(())` without killing in the lobby.

## Risks / Trade-offs

- [The signature change touches ~125 sites] → Mechanical. The compiler lists them, and most frame and modeline readers render an empty segment. Land it as the first task with no behaviour change other than `Core::default()` still making a workspace. Then switch launch.
- [App and GPUI tests and `boundaries.rs` assume a launch workspace] → They call `core.launch(root, root)`, which still yields one workspace. Tests with no `launch` move to an explicit workspace or to lobby expectations.
- [Agents or terminals reaching the lobby through a stale timer or reply] → Their buffers belong to killed workspaces and are gone. Replies for missing buffers are already dropped. A lobby test kills the last workspace while a terminal and the fake agent are live.
- [Users relying on `C-x t 0` refusing the sole workspace as a safety net] → The save and process questions still guard data, and the lobby is one `RET` from a new workspace.

## Migration Plan

There is no persisted state, so nothing to migrate. Update `docs/editor.md`:
- Projects → "Launching without a project": the no-argument bullet supersedes #189's `$HOME` rule.
- Workspaces: the sole-workspace kill and the lobby.

## Open Questions

None that change specs or tasks. The lobby hint wording can be tuned during visual review against frames 13–13d.
