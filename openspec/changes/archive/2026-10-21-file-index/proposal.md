# Proposal

## Why

The file finder across project roots (#70), and later the watcher (#80), need every file of a project without walking the disk on each keystroke. Walking a large repo on the core thread would freeze typing, so the walk has to run in a service and its result has to be kept in memory (#68).

## What Changes

- A file index per project: the files under each of its roots, as paths relative to the root, held in memory only and rebuilt on demand. Nothing is written to disk.
- The walk honours `.gitignore` (in a git repo or not), `.ignore` and git's global excludes, includes hidden files, skips `.git` and does not follow symlinked directories.
- The walk runs on the file service's blocking pool; the core only stores the answer, so typing goes on while a large repo is indexed.
- A new command `project-index`, bound to `C-x p i`, prompts `Project: ` and rebuilds that project's index, echoing `Indexing <project>...` and then `Indexed <n> files in <project>`, with any root that could not be walked named after it.
- A rebuild started while another is running wins: the older answer is dropped.

Not in this change: the finder that reads the index (#70), building the index without the command, refreshing on file events (#80), project ignore patterns (#190).

## Capabilities

### New Capabilities
- `file-index`: the per-project index of files under the project's roots, what it includes, and how it is rebuilt.

### Modified Capabilities
- `projects`: `project-index` joins the project commands that run through `M-x`, are bound under `C-x p` and prompt `Project: ` first.

## Impact

- `aiditor-core`: `FileOp::Index` and `Msg::Indexed`, the index on `Core`, the `project-index` command and its binding, the harness.
- `aiditor-services`: the walk in `files.rs`; new dependency `ignore` (ripgrep's walker, already chosen in `docs/editor.md`, and reused by grep #73).
- `docs/editor.md`: the file index recorded under Subsystems.
