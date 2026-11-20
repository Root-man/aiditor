# Proposal

## Why

The diff view's status mode (M9) lists every git root's staged and unstaged changes under a heading naming its branch. It needs one service operation that asks git for that, per root, off the core thread. This is the first of the M9 git operations (#500).

## What Changes

- New file service operation `FileOp::GitStatus(roots)`, answered by `Msg::GitStatus`: for each root, its repository's root, its branch as the mode line names it, and its changed files from `git status --porcelain=v2 -z --untracked-files=all`, limited to the root. Each file carries its path, staged state, unstaged state, a rename's or copy's old path; untracked files are an unstaged state of their own.
- A root with no `.git` at or above it answers that it isn't a repository; a git failure answers its error. Neither stops the other roots, which run concurrently.
- The status runs as the mode line's ` ±` does: no optional locks, `core.fsmonitor` off, and refused when the repository's own config defines a content filter.
- The core ignores the answer until the diff view asks.

## Capabilities

### New Capabilities
- `git-status`: a project's roots' branches and changed files, from git.

### Modified Capabilities
None.

## Impact

- `crates/aiditor-core/src/lib.rs`: `FileOp::GitStatus`, `Msg::GitStatus`, `GitStatus`, `GitFile`, `GitChange`; a no-op arm in the core's update; the harness leaves the op as an effect.
- `crates/aiditor-services/src/files.rs`: the operation, a porcelain v2 parser, `head()` split so a directory's repository and branch can be found; the repository-filter check shared with `changed()`.
- `docs/editor.md`: the Git subsystem records the operation.
- No new dependencies.
