# Proposal

## Why

The diff view (#504) lists changed files beside an empty right window. #505 fills it: the file at point in the list, drawn as an inline diff over a buffer, editable where the new side is the working tree, so code can be fixed while it is reviewed.

## What Changes

- Moving point onto a file in the list shows its diff in the right window: under `Changes` index → working tree over the file's own buffer (visited if not open), under `Staged` `HEAD` → index over a read-only buffer holding the index text.
- Removed lines are diff rows above what replaced them, in `diff-removed`, labelled with their old line numbers; added lines carry a `diff-added` bar (backgrounds and the sign column are #508's).
- The view holds the old text; a moment after each edit or revert of the shown buffer it diffs again off the core thread, a stale answer dropped.
- Commands: next/previous change (`C-c n`/`C-c p`, `n`/`p` in the list), next/previous file (`C-c f`/`C-c b`, `f`/`b` in the list), copy the change's removed lines (`C-c w`, `w`), and `C-g` quits the view leaving the file's buffer at point (the user's choice over `RET`, which edits).
- A binary, non-UTF-8 or oversized file shows a one-line reason in the right window.
- The diff window's mode line adds the added/removed counts, the sides compared and which change point is in.
- The `diff-view` keymap no longer applies in the minibuffer, so `C-g` there still aborts.

## Capabilities

### New Capabilities

### Modified Capabilities
- `diff-view`: the inline diff of the file at point, its commands and mode line.

## Impact

`aiditor-core`: `diff.rs` (the shown diff's state, requests and answers, commands), `lib.rs` (dispatching the git and diff answers, a re-diff timer message), `frame.rs` (added-line bars and mode line segments), `bindings.rs` (minibuffer exclusion), `visiting.rs` (an `Opened` for the view), `builtins.rs`, `keymap.toml`. `docs/editor.md`.
