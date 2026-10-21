# Proposal

## Why

Since #62 and #63 the core has workspaces with their own buffers and layouts, but nothing a user can press: no command makes, switches or kills one, and the launch workspace `default` is bound to no project, against the data model (every workspace belongs to one project). This change (#64) adds the workspace commands, open folder and switch project, and the launch rules that "Decide: launching without a project" (#189) settled.

## What Changes

- Workspace commands under the `C-x t` prefix and through `M-x`: `workspace-new` (prompts for a project), `workspace-switch`, `workspace-next`, `workspace-previous`, `workspace-rename`, `workspace-kill` and `workspace-list`.
- `workspace-kill` asks about the workspace's modified file buffers the way `C-x C-c` does, and kills once the chosen saves succeed.
- `workspace-open-folder` (`C-x t d`): a folder no saved project contains gets an ad-hoc in-memory project (the folder as primary, named after it) and a workspace for it; nothing is written.
- `project-switch` (`C-x p p`): shows the project's most recently shown workspace, or makes one when it has none.
- Launch: a file on the command line opens in a workspace of the first saved project whose roots contain it, else of an ad-hoc project at its nearest git repo or else its folder; a folder on the command line is opened as a folder; no argument gives a workspace on an ad-hoc `$HOME` project.
- **BREAKING** (internal): the projectless `default` workspace goes; a workspace's project is required.
- Reloading projects (`C-x C-r`) keeps the ad-hoc projects, since workspaces are bound to them.

## Capabilities

### New Capabilities

### Modified Capabilities
- `workspaces`: the launch workspace is bound to a project; adds the workspace commands, kill asking about modified buffers, open folder and the launch rules.
- `projects`: a reload keeps ad-hoc projects; adds `project-switch`.

## Impact

`aiditor-core`: `workspace.rs` (commands, required project, last-shown order, launch), `builtins.rs` (the new commands), `visiting.rs` (the `C-x C-c` questions shared with kill), `lib.rs` (`ProjectsLoaded` keeps ad-hoc projects, the launch workspace), `project_commands.rs` (finding or making the project for a path), `keymap.toml`. `aiditor`: `main.rs` passes the file's git repo or folder to the core and accepts a folder argument. `docs/editor.md`: "Built in #64" notes under Workspaces and Projects.
