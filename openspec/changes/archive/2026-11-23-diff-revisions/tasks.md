# Tasks

## 1. File service

- [x] 1.1 Add `FileOp::GitRefs` / `Msg::GitRefs` in `aiditor-core` and answer them in `aiditor-services` (`for-each-ref`), with a services test of branches, tags and a non-git root

## 2. Core

- [x] 2.1 Hold sides in `Shown`, replace `staged()` uses with the sides, and build the mode line's sides label from them; existing diff tests pass
- [x] 2.2 `diff-revisions` command: refs ask, root/old/new prompts, `GitDiffFiles` ask, open or echo on answer; harness tests for one and several git roots, empty old revision, unknown revision
- [x] 2.3 Render a revisions list (heading with revisions, files under root, folding), show diffs with tree sides (read-only `<path> (<new>)`) or the working tree buffer; tests for two revisions and the working tree
- [x] 2.4 Refuse stage/unstage/discard, refresh via `GitDiffFiles`, replace a view of the other kind; tests

## 3. Docs

- [x] 3.1 Record "Revisions as built" in `docs/editor.md` and verify it matches the specs
