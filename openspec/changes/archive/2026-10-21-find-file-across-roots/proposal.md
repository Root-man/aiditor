# Proposal

## Why

`C-x C-f` only browses one directory at a time, so a file in an additional root (`~/Repos/zed` next to the primary repo) means typing its whole path. The file index (#68) already holds every root's files; #70 adds the finder that matches over them.

## What Changes

- A new command `project-find-file`, bound to `C-x p f`, for the current workspace's project. It prompts `Find file in <project>: ` and completes, with `nucleo`, over every indexed file as `<root name>/<path relative to the root>`, the primary's files first. `RET` visits the selected file.
- A root's name is its directory's last component; a name an earlier root of the project already has gets `<2>`, `<3>`, as buffer names do.
- Index on first use: when the project has no index yet, opening the finder starts the walk, and its files join the open list when it answers. Later opens use the kept index; `C-x p i` rebuilds it.
- Text matching no candidate is visited under the root its first component names, or under the primary when it names none, so a new file can be made.
- Live preview for `project-find-file` and `C-x C-f`: the selected file shows in the window the prompt was opened from. A file not yet visited is read off the core thread into one hidden, read-only preview buffer, killed when the next one is read and when the minibuffer closes, as consult does. Recorded in `docs/editor.md`.

Not in this change: highlighting matched characters (#201), a size limit on previewed files, `M-p` history in the finder, refreshing the index on file events (#80), project ignore patterns (#190).

## Capabilities

### New Capabilities
- `file-finder`: finding a file across all roots of the current workspace's project.

### Modified Capabilities
- `projects`: `project-find-file` joins the project commands bound under `C-x p`, and does not ask for a project.
- `minibuffer`: a preview can name a file, read into a preview buffer; a command with no declared prompts previews its own question.
- `file-visiting`: `C-x C-f` previews the selected file.

## Impact

- `aiditor-core`: the `project-find-file` command and its binding in `keymap.toml`; `indexed` adds files to an open finder's list; the preview hook names a buffer or a file; `FileOp::Preview` and `Msg::Previewed`.
- `aiditor-services`: answers `FileOp::Preview` with the same read as `FileOp::Open`.
- `docs/editor.md`: the finder and file preview under Projects / Subsystems.
