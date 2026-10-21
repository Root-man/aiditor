# Design

## Context

`workspace.rs` has `new_workspace`, `switch_workspace` and `kill_workspace` as core calls with no commands. `Workspace.project` is `Option<String>`, `None` only for the launch workspace `default` that `Core::default` makes. Projects live in `Core.projects: Vec<Loaded>`, ad-hoc ones with `file: None`; `Msg::ProjectsLoaded` clears the list. `C-x C-c` (`save_buffers_kill_terminal`) chains its questions as name/answer string pairs and waits for its saves through `Core.quitting`.

## Goals / Non-Goals

**Goals:** every workspace has a project from launch on; the commands reuse the minibuffer chaining and the `C-x C-c` question flow rather than adding new ones.

**Non-Goals:** the workspace strip (its own issue), file watching (no watcher yet, so "watching off for `$HOME`" needs nothing), dropping an ad-hoc project when its last workspace dies, scoping `C-x b` to the workspace (#66).

## Decisions

- **Required project.** `Workspace.project: String`. `Core::default` binds the launch workspace to an ad-hoc project at `std::env::home_dir()` (`/` when unknown), named after its last component. Alternative: an `Option` until launch, rejected since "no workspace without a project" is the point.
- **Launch.** `Core::launch(path, root)` rebinds the sole launch workspace (and renames it) to the project found or made for `path`, falling back to `root`, then drops the `$HOME` project when nothing else uses it. `main.rs` computes `root`: for a file, the nearest ancestor holding `.git`, else its folder; for a directory, the directory itself, and no file is read. The `.git` walk stays in the app, keeping filesystem probes out of the core. Rebinding beats making a second workspace and killing the first: no ids or windows change.
- **Finding a path's project** (`project_for(path, root)`): the first saved project with a root that `path` starts with; else the ad-hoc project whose primary is exactly `root`; else a new ad-hoc project at `root`. Ad-hoc projects are not matched by containment, or the `$HOME` project would swallow every folder. Taken names get `<2>`, `<3>` like buffers.
- **Reload keeps ad-hoc projects**: `ProjectsLoaded` keeps `file: None` entries and replaces the rest, so workspaces stay bound across `C-x C-r`.
- **Last shown.** `Workspace.shown: u64`, set from a core counter in `show_workspace` and at creation. `workspace-switch` sorts by it and `project-switch` takes the max.
- **Kill reuses the quit questions.** `save_buffers_kill_terminal` becomes a shared `save_some(command, args, skip, of, verb)` that walks the pairs after `skip` leading strings, only over the buffers `of` accepts (the workspace's own for kill, all for quit), and returns the buffers to save, or nothing while it is still asking or was declined. Quit and kill differ only in what happens when the saves finish: `Core.quitting` keeps the buffers waited on and a new `Core.then_kill: Option<WorkspaceId>` says to kill rather than quit.
- **Keys.** `C-x t`, Emacs's tab-bar prefix, the closest Emacs analogue (a tab is a window configuration): `2` new, `RET` switch, `o`/`O` next/previous, `r` rename, `0` kill, `l` list, `d` open folder; `C-x p p` is Emacs's `project-switch-project`.

## Risks / Trade-offs

- A reload that drops a saved project leaves its workspaces bound to a missing name → project commands then say `No project named <name>`; acceptable until a reload keeps bound projects.
- A quit started while a kill waits on its saves replaces the kill's wait (one `quitting` list) → rare, and the saves still run.
- No visible sign of the current workspace until the strip lands → `workspace-list` shows it.
