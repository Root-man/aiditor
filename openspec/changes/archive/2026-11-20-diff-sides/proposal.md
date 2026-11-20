# Proposal

## Why

The diff view (M9) draws one file's diff between two sides of a repository, and re-draws it as the file's buffer is edited. It needs the service operations that read a file on each side, work out its changed lines off the core thread, and list the files that differ between two revisions for the revisions and agent turn modes (#501).

## What Changes

- `FileOp::GitDiff`, answered by `Msg::GitDiffed`: one file's text on two sides of a repository, where a side is a tree-ish, the index or the working tree, and its changed line ranges (old and new), from `similar` as a revert diffs. A file missing on a side is empty there; a binary or non-UTF-8 file answers a reason and no text.
- `FileOp::DiffLines`, answered by `Msg::LinesDiffed`: the same ranges between a held old text and a snapshot of a buffer's text, so a working tree being edited is diffed as its buffer holds it.
- `FileOp::GitDiffFiles`, answered by `Msg::GitDiffFiles`: the files that differ between two tree-ishes, or a tree-ish and the working tree, under a root, from `git diff --name-status -z`.
- Both git operations can be given an object directory and alternates, so a snapshot tree written outside the repository's objects can be read.
- They run with the status operation's safety flags; a tree-ish starting with `-` is refused, and listing against the working tree is refused when the repository's own config defines a content filter.
- The core ignores the answers until the diff view asks.

## Capabilities

### New Capabilities
- `git-diff`: one file's diff between two sides of a repository, a buffer's re-diff, and the files that differ between two sides.

### Modified Capabilities
None.

## Impact

- `crates/aiditor-core/src/lib.rs`: `GitSide`, `GitObjects`, `FileDiff`, `Hunk`, `ChangedFile`, the three ops and messages, no-op core arms; the harness leaves the ops as effects.
- `crates/aiditor-core/src/buffer.rs`: `hunks()`, the changed line ranges between two texts.
- `crates/aiditor-services/src/files.rs`: the operations, a `--name-status -z` parser, a shared `git()` command with the safety flags, the status letter parser shared with `porcelain_v2`.
- `docs/editor.md`: the Git subsystem records the operations.
- No new dependencies.
