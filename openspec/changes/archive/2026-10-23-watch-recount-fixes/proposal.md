# Proposal

## Why

Review of #191 and #192 found that any project edit (role, default agent, root) or `C-x C-r` dropped the project's root watches and showed `[unwatched]` until a full re-walk answered, because the index compared a whole copy of the project and reloads cleared its readiness. On Linux each drop re-adds every directory's inotify watch, and changes in the gap start no re-walk. Every directory report also started a walk, though only empty directories change the counts.

## What Changes

- The index's counts belong to the walked roots and ignore patterns only; a re-walk of the same ones keeps the counts, and the root watches, until it answers.
- Only a reported directory that holds none of the index's files starts a walk, once.
- `project-find-file` fills from a pending walk only when the index has no answer yet.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `file-watching`: re-walks of unchanged roots and patterns keep root watches.
- `file-index`: directory reports.

## Impact

`aiditor-core` watching and project index code, their tests, and `docs/editor.md`.
