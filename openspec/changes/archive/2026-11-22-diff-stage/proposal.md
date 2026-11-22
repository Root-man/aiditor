# Proposal

## Why

The diff view lists what changed but can't act on it, so reviewing an agent's work still ends in a terminal running `git add` and `git checkout` file by file. Issue #510: stage, unstage and discard files from the changed-file list.

## What Changes

- `s` (`diff-stage`), `u` (`diff-unstage`) and `k` (`diff-discard`) in the changed-file list act on the file at point, or on every file under the `Staged`/`Changes` heading or root heading at point.
- Staging adds the `Changes` files to the index; unstaging puts the `Staged` files back to `HEAD` in the index (a rename's old path too); discarding restores a `Changes` file from the index and deletes an untracked one, after a confirmation naming what is lost. Discarding a `Staged` file unstages it, as #511 does for a `Staged` diff, so on a root heading `k` discards the `Changes` files and unstages the `Staged` ones.
- Staging or discarding a file whose buffer has unsaved edits is refused, since git sees the disk.
- A success refreshes the list; a failure echoes git's message and leaves the list as it was. Open buffers pick up a discard through the file watcher.
- A refresh now asks the shown diff again, and when point's row is gone it goes to the nearest file that was listed after it (else before it) instead of the first file, so `s s s` walks down `Changes`.
- The list's key hints add `s/u stage/unstage` and `k discard`.

## Capabilities

### New Capabilities
- `git-staging`: the file service's operation that stages, unstages, restores and deletes files of a repository in order, answering git's error.

### Modified Capabilities
- `diff-view`: the list's rows also change after a stage, unstage or discard; a refresh re-asks the shown diff and keeps point near a row that left; new requirements for the three actions and their hints.

## Impact

- `crates/aiditor-core/src/lib.rs`: `FileOp::GitUpdate`, `Msg::GitUpdated`, `GitStep`.
- `crates/aiditor-core/src/diff.rs`: the three commands, refresh changes, tests.
- `crates/aiditor-core/src/builtins.rs`, `keymap.toml`, `harness.rs`.
- `crates/aiditor-services/src/files.rs`: running the steps, tests against real repositories.
- `docs/editor.md`: "Staging as built".
