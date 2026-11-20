# Proposal

## Why

M9's diff view needs its frame before it can show diffs: a temporary full-frame layout of the current workspace with the changed-file list on the left, a window for the diff on the right and one mode line under both (#504, frames 15 and 15c).

## What Changes

- `diff-status` saves the current workspace's window layout and shows two side-by-side windows: a narrow `*Diff*<workspace>` list (a new read-only buffer kind) and, until the inline diff lands (#505), the buffer that was current.
- The list asks git status for every root of the project and shows each root as a bold heading with its branch, its `Staged` and `Changes` groups with counts, and each file's status letter and path relative to the root; a clean root says `No changes`, a root outside git `not a git repository`, a failure its error.
- The list refreshes only on `g` (or `C-c g` anywhere in the view), keeping point on its file.
- `q` in the list, or `C-c q` anywhere in the view, quits: the saved layout comes back and the list is killed; other buffers stay open.
- The list window draws no mode line; the window right of it draws its mode line under both.
- Running `diff-status` again while the view is open selects the list.
- New faces `diff-file-modified`, `diff-file-added`, `diff-file-deleted`, `diff-file-untracked` and `diff-list-group`.

## Capabilities

### New Capabilities
- `diff-view`: the diff view's layout, its changed-file list, refreshing and quitting.

### Modified Capabilities

## Impact

`aiditor-core`: `workspace.rs` (a saved layout in `Windows`), new `diff.rs`, `buffer.rs` (list kind), `frame.rs` (one mode line), `bindings.rs` (the view's keymap layer), `builtins.rs`, `keymap.toml`, `settings.rs`/`settings.toml` (faces), `lib.rs` (`Msg::GitStatus` routing). `aiditor`: `element.rs` paints a window's mode line where the core puts it, or none. `docs/editor.md`.
