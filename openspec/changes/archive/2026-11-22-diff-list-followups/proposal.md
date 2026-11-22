# Proposal

## Why

Review of #508 (epic #503) found three rough edges in the diff view: the `Staged`/`Changes` counts don't line up because the group names differ in length, a root folds only as a whole so a long `Changes` group can't be put away while `Staged` stays open, and the 300 ms pause before a re-diff makes a typed line visibly turn green late.

## What Changes

- A group heading's count is right-aligned in a fixed column wide enough for 3 digits, so counts line up across groups and roots.
- `Staged` and `Changes` fold separately: a group heading has its own `▾`/`▸` mark, `TAB` on a group heading or on a file under it folds or unfolds that group, and `TAB` on a root heading still folds the root. Group folds last across refreshes, as root folds do. **BREAKING** (behaviour): `TAB` on a file row now folds its group, not its root.
- The re-diff after an edit or revert of the shown buffer waits 50 ms instead of 300 ms.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `diff-view`: the list's group headings (fold mark, aligned count), folding (per group as well as per root), and what may change the list's rows.

## Impact

- `crates/aiditor-core/src/diff.rs`: list rendering, `diff_toggle`, the folded set, `REDIFF`, tests.
- `docs/editor.md`: "Look as built" (folding, counts) and "Inline diff as built" ("300 ms").
- No new dependencies, settings or keys.
