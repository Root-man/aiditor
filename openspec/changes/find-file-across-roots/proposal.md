# Proposal

## Why

`C-x C-f` only browses one directory at a time, so a file in an additional root (`~/Repos/zed` next to the primary repo) means typing its whole path. The file index (#68) already holds every root's files; #70 adds the finder that matches over them.

## What Changes

- A new command `project-find-file`, bound to `C-x p f`, for the current workspace's project. It prompts `Find file in <project>: ` and completes, with `nucleo`, over every indexed file as `<root name>/<path relative to the root>`, the primary's files first. `RET` visits the selected file.
- A root's name is its directory's last component; a name an earlier root of the project already has gets `<2>`, `<3>`, as buffer names do.
- Index on first use: when the project has no index yet, opening the finder starts the walk, and its files join the open list when it answers. Later opens use the kept index; `C-x p i` rebuilds it.
- Text matching no candidate is visited under the root its first component names, or under the primary when it names none, so a new file can be made.
- No live preview: the issue asked for one, but the user decided neither `C-x C-f` nor `project-find-file` previews. Recorded in `docs/editor.md`.

Not in this change: preview, `M-p` history in the finder, refreshing the index on file events (#80), project ignore patterns (#190).

## Capabilities

### New Capabilities
- `file-finder`: finding a file across all roots of the current workspace's project.

### Modified Capabilities
- `projects`: `project-find-file` joins the project commands bound under `C-x p`, and does not ask for a project.

## Impact

- `aiditor-core`: the `project-find-file` command and its binding in `keymap.toml`; `indexed` adds files to an open finder's list.
- `docs/editor.md`: the finder and the no-preview decision under Projects / Subsystems.
