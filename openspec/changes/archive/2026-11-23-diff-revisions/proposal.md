# Proposal

## Why

M9's diff view shows only the status diff. Comparing two branches, a tag and a commit, or a revision against the working tree (#513) needs the same view over `git diff A B` of one root.

## What Changes

- New command `diff-revisions`: asks for a git root when the project has several, then the old and the new revision, completing over that root's branches and tags and taking any text git resolves (`M-RET` takes the typed text). An empty new revision is the working tree.
- The view opens only once git has listed the differing files; an unknown revision echoes git's error and opens nothing.
- The list shows the root's differing files straight under its heading (no `Staged`/`Changes`), the heading naming both revisions; the mode line's sides read `<old> → <new>` or `<old> → working tree`.
- Against the working tree the new side is the file's own buffer, editable as in a status diff; between two revisions it is a read-only buffer of that revision's text.
- Stage, unstage and discard echo `This diff can't be changed`.
- Refresh lists the files again, so git resolves the revisions again.
- The file service gains a listing of each root's repository, branches and tags.

## Capabilities

### New Capabilities
- `diff-revisions`: comparing two revisions of one root, or a revision and the working tree, in the diff view.

### Modified Capabilities
- `git-diff`: adds listing a root's repository, branches and tags.

## Impact

`aiditor-core` (`diff.rs`, `lib.rs` messages and file ops, `builtins.rs`, harness), `aiditor-services` (`files.rs`), `docs/editor.md` (diff view, revisions).
