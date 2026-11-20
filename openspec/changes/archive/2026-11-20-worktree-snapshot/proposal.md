# Proposal

## Why

The agent turn diff (M9) compares every git root as it was when the session's last prompt was sent with the working tree now. Agents edit mostly through shell commands, so the "before" side must be a snapshot of the whole working tree, taken without touching the repository (#502).

## What Changes

- `FileOp::GitSnapshot`, answered by `Msg::GitSnapshot`: a root's repository, the id of a tree holding its working tree as it is on disk (modified, staged and untracked files; no ignored ones), and the object directory and alternates that hold the tree, ready for `GitDiff` and `GitDiffFiles`. A root outside a repository answers that it isn't one.
- The tree is written with a copy of the repository's index as `GIT_INDEX_FILE`, `git add -A` and `git write-tree`, objects going to a directory in the editor's data dir with the repository's objects as alternates. The repository's index, refs, objects and working tree are left byte-for-byte unchanged.
- The snapshot index is kept per repository as the next snapshot's seed, so an unchanged file is not read again.
- Each editor process keeps its snapshots in its own directory under the data dir; at launch, the directories of processes no longer running are removed, so the object directory starts empty.
- A benchmark records a snapshot of a tree with a few hundred changed files.
- Nothing asks for a snapshot yet.

## Capabilities

### New Capabilities
- `git-snapshot`: a tree id for a root's working tree, written outside the repository.

### Modified Capabilities
None.

## Impact

- `crates/aiditor-core/src/lib.rs`: `Snapshot`, the op and message, a no-op core arm; the harness leaves the op as an effect.
- `crates/aiditor-services/src/files.rs`: `snapshot()`, `clear_snapshots()`, tests.
- `crates/aiditor/src/main.rs`: clears dead processes' snapshot directories at launch.
- `crates/aiditor-services/benches/services.rs`, `docs/performance.md`: the benchmark.
- `docs/editor.md`: the Git subsystem records the operation and the measured time.
- No new dependencies.
