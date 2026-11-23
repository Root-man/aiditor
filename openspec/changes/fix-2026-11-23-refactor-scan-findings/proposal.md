# Proposal

## Why

The 2026-11-23 refactor scan of the whole repo found 10 problems. One is a bug in the diff view: a second stage, unstage or discard can start while git is still running the first, and the two collide on `index.lock`. The rest is duplication and leftovers in code that the sessions list rework (#468) and the agent turn diff (#514) change next. Fixing them now, while the findings are fresh, is cheaper than working around them in those changes.

## What Changes

Each finding keeps its row number from the scan.

**Bug**
- **#1 One git update at a time in the diff view:** while a stage, unstage or discard is running in git, or the list it refreshes hasn't arrived yet, `diff-stage`, `diff-unstage` and `diff-discard` are refused, echoing `Git is still updating`. Today the second one runs at once on another blocking thread. It can fail with git's `index.lock` message, or build a patch against the index from before the first update.

**Lean and organization (no behavior change)**
- **#2 One list writer:** `render_sessions` and `render_diff_list` share two `Core` helpers. One finds point in the list, the other writes the lines with their properties and moves point in every window showing the list.
- **#3 One fold lookup in `display::build`:** the main loop and `rest` stop repeating the `Invisible`/`Display` → placeholder match.
- **#4 `git.rs` in services:** the git functions move from `files.rs` to `aiditor-services/src/git.rs`; `files.rs` keeps file IO and indexing. Moves only.
- **#5 `Windows::iter()`:** the 6 hand-written walks over a layout's windows use it, next to the existing `windows_mut()`.
- **#6 Core `pub` trimmed:** the 37 `pub fn`s in public core modules that no other crate uses become `pub(crate)`. Any that rustc then reports as dead are deleted.
- **#7** The duplicate `row` closure in `Editor::render` is deleted.
- **#8 One git runner:** a helper runs a git command and returns its stdout or its stderr as the error. It replaces `check`, `snapshot`'s `run` closure and the same check written out in `git_status`, `git_refs` and `git_diff_files`.
- **#9** The stale doc comment on `Core::list_requests` is deleted.
- **#10 `prepaint` split:** the terminal-cell block moves out of `FrameElement::prepaint` into its own function, and one helper builds the cursor face that both cursor paths use.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `diff-view`: stage, unstage and discard are refused while an earlier one is still updating (#1).

The other findings change no behavior; their existing tests and specs must keep passing unchanged.

## Impact

- Core (`aiditor-core`): `diff.rs` (#1, #2, #5), `sessions.rs` (#2, #5), `display.rs` (#3), `workspace.rs` (#5), `frame.rs` and `workspace_list.rs` (#5), `buffer.rs`, `window.rs`, `display.rs`, `settings.rs`, `terminal.rs`, `lib.rs` (#6, #9).
- Services (`aiditor-services`): `files.rs` and a new `git.rs` (#4, #8), `lib.rs` (`pub mod git`), `benches/services.rs` (`git::snapshot`).
- Frontend (`aiditor`): `main.rs` (`git::clear_snapshots`), `editor.rs` (#7), `element.rs` (#10).
- Docs: `docs/refactoring-opportunities.md` gets this scan's record, as the earlier scans have.
