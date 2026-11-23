# Design

## Context

The file service already answers `GitDiffFiles` (files between two tree-ishes or a tree-ish and the working tree, paths from the repository top) and `GitDiff` with `GitSide::Tree` sides; the core ignores `Msg::GitDiffFiles`. The view (`diff.rs`) renders a status answer (`Vec<(root, Result<Option<GitStatus>>)>`) into rows and shows a row's diff with sides picked from its `staged` flag.

## Goals / Non-Goals

**Goals:** reuse the status view whole: rows, folding, navigation, inline diff, re-diff, highlighting.

**Non-Goals:** several roots in one revisions diff, remote branches in completion, a revision log picker.

## Decisions

- **One new file op, `GitRefs(roots)`**, answering per root `Option<(repository, refs)>` via `git for-each-ref refs/heads refs/tags`. It gives the git roots (for the root prompt), the completion and the repository top `GitDiff` needs, in one round trip before the first prompt. Alternative: `Head` on roots plus a refs op — two ops for one need.
- **The command asks its own questions** with `read_string` and choices, as `diff-discard` does; the refs answer opens the first prompt. The spec is `unselected`, so `RET` on an empty new revision answers empty (the working tree) and `M-RET` takes typed text like `HEAD~3`.
- **The view opens on the `GitDiffFiles` answer**, so an unknown revision echoes git's error and the layout is untouched. The pending ask is kept in the core with its workspace; an answer for another workspace or a newer ask is dropped.
- **The answer is converted to the status answer's shape**: one root, a `GitStatus` whose files are `unstaged` changes. `DiffList` gains `revisions: Option<(old, new)>`; rendering skips the group headings when set and the heading names the revisions instead of the branch. Rows stay `Row::File { staged: false }`.
- **`Shown` holds its sides** (`GitSide` pair) instead of deriving them from `staged`; "the new side is fixed" is `new != WorkingTree`, replacing `staged()` for read-only side buffers, re-diffing and `diff-visit`; the mode line's sides label is built from the pair (`HEAD → index`, `index → working tree` unchanged).
- **`diff_update` refuses first** when the list has revisions, before either window's path.

## Risks / Trade-offs

- [Fuzzy completion can pick a ref while typing a commit hash] → `M-RET` takes the typed text; documented.
- [Rows hold `staged: false`, so code keyed on it treats revisions as `Changes`] → staging is refused before that code runs; the read-only decision uses the sides.
