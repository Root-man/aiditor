# Design

## Context

#500 and #501 added `git status`, file diffs and file listings in `files.rs`, with a shared `git(dir)` command (`--no-optional-locks`, `--literal-pathspecs`, `core.fsmonitor=false`), `no_filter()` and `GitObjects`, which `GitDiff` and `GitDiffFiles` already take for snapshot trees. `repository(dir)` finds the nearest `.git`, following a `.git` file as linked worktrees and submodules have.

## Goals / Non-Goals

**Goals:** a tree id for a root's working tree without touching the repository; an unchanged file hashed once across snapshots; a clean object directory per launch; a benchmark.

**Non-Goals:** holding snapshots per session or per prompt (the core's job in the turn diff task), reverting to a snapshot, a snapshot of a root outside a repository (a shadow git directory could add that later), concurrent instances sharing one snapshot.

## Decisions

- **Op.** `FileOp::GitSnapshot { request, root, dir }` answers `Msg::GitSnapshot { request, result: Result<Option<Snapshot>, String> }`, `None` outside a repository as `GitStatus` does. `Snapshot { repository, tree, objects: GitObjects }` carries what `GitDiff`/`GitDiffFiles` need. `dir` is the process's snapshot directory, passed in as the recovery dir is, since the services don't know the data dir.
- **Paths.** `repository(root)` decides whether the root is in a repository and gives its top directory, as status does. The repository's own index and object directory come from `git rev-parse --path-format=absolute --git-path index --git-path objects`, which handles a linked worktree's per-worktree index and shared objects.
- **Writing.** In the repository's top directory, with `GIT_INDEX_FILE` set to a private copy of the seed and `GIT_OBJECT_DIRECTORY` to `<dir>/objects`, the repository's objects as `GIT_ALTERNATE_OBJECT_DIRECTORIES`: `git add -A`, then `git write-tree`. `-A` with no pathspec covers the whole repository, so the tree's paths are relative to its top, as `GitDiffFiles` expects. `core.splitIndex=false` keeps git from writing a shared index file next to the repository's.
- **Seed.** The kept index is `<dir>/indexes/<FNV-1a hash of the repository path>`; without one, the repository's index is the seed. Each snapshot copies the seed to a uniquely named file, runs git on the copy, and renames it over the kept index, so two snapshots of one repository at once don't share a lock and the later one wins. On failure the copy is removed. The kept index's stat cache means `git add -A` reads only files whose stat changed.
- **Safety.** The repository's own config defining a content filter answers an error, as status does, since `git add` runs clean filters. The same `git(dir)` flags apply.
- **Clearing.** `<data dir>/snapshots/<pid>` per process. `files::clear_snapshots(parent)` removes every `<pid>` directory whose process is gone (`kill(pid, 0)` failing with `ESRCH`); the app runs it on a thread at launch. Kept indexes go with their objects, since an index entry whose blob is gone would make `write-tree` fail.
- **Benchmark.** `snapshot/300-changed-files-*` in the services bench: a 2,000-file repository with 300 modified files and 20 untracked ones; a first snapshot (no kept index) and a repeat (kept index).

## Risks / Trade-offs

- A snapshot directory grows for the life of the process; nothing prunes objects until the next launch. Fine while sessions are short; prune unreachable objects if it shows.
- A pid reused by another program keeps a dead process's directory until that program exits.
- A submodule is snapshotted as the commit its gitlink names, not its working tree.
