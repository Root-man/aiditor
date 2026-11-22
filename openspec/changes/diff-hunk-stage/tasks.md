## 1. File service

- [x] 1.1 Add `GitStep::Apply(String)` (drop `Copy`) and run it in `git_update` as `git apply --cached -` with the patch on stdin, kept by the empty-path filter
- [x] 1.2 Add `buffer::patch(file, base, edited)`: the unified diff with a `diff --git` header
- [x] 1.3 Service tests on real repositories: one change of several, changes at a file's start and end without a final newline, a patch that doesn't apply

## 2. Core

- [x] 2.1 `diff-stage`/`diff-unstage`/`diff-discard` in the diff window: the change at point or the region's lines, the wrong action's message, unsaved edits and stale hunks refused, whole files for added/deleted/untracked/renamed/copied
- [x] 2.2 Stage and unstage send a `GitUpdate` with the patch; discard in `Changes` edits the buffer as one undo group; the region is deactivated
- [x] 2.3 Keep point in the diff window across a refresh of the same row
- [x] 2.4 `C-c s`, `C-c u`, `C-c k` in `[diff-view]`
- [x] 2.5 Harness tests: one change of several, a region, unstage, discard and `C-/`, unsaved edits, the wrong action, an untracked file, point kept

## 3. Docs

- [x] 3.1 `docs/editor.md`: "Change staging as built" replacing the #510 paragraph's forward note
