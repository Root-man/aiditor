# Proposal

## Why

The list stages, unstages and discards whole files (#510), but reviewing an agent's turn often means keeping one change of a file and dropping another. Issue #511: stage, unstage and discard the change at point in the diff, or only a region's lines.

## What Changes

- In the diff window, `C-c s` (`diff-stage`), `C-c u` (`diff-unstage`) and `C-c k` (`diff-discard`) act on the change at point, or with an active region on the region's added lines and the removed rows of every change it touches. In the list the same keys act on files as `s`, `u` and `k` do.
- Staging (in a `Changes` diff) and unstaging (in a `Staged` diff) build a patch against the index and the file service applies it to the index alone. Discarding in a `Staged` diff unstages.
- Discarding in a `Changes` diff edits the file's buffer, not the disk: the change goes back to the old side's text as one undo group, without a question since `C-/` brings it back, and the buffer is left unsaved.
- Staging refuses while the file's buffer has unsaved edits (`Save <path> first`); the wrong action for the diff echoes why (`Already staged`, `Not staged`).
- A file added, deleted, untracked, renamed or copied on the diff's sides is one change and is staged or unstaged whole, as from the list.
- After a stage or unstage the list refreshes and point in the diff window stays where it was instead of going to the first change.

## Capabilities

### New Capabilities
None.

### Modified Capabilities
- `git-staging`: a step that applies a patch to the index.
- `diff-view`: acting on a change or a region from the diff window, and point kept across refreshes.

## Impact

- `crates/aiditor-core/src/lib.rs`: `GitStep::Apply`.
- `crates/aiditor-core/src/buffer.rs`: building a patch.
- `crates/aiditor-core/src/diff.rs`: the commands in the diff window, point kept across refreshes, tests.
- `crates/aiditor-core/keymap.toml`: `C-c s`, `C-c u`, `C-c k` in `diff-view`.
- `crates/aiditor-services/src/files.rs`: applying a patch, tests against real repositories.
- `docs/editor.md`: "Change staging as built".
