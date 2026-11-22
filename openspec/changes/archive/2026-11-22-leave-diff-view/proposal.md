# Proposal

## Why

The diff view is a temporary layout, but other commands can still rearrange it: `C-x p f` in the list opens the file in the list's window, and after `C-x 0` on the diff window no file's diff can show again. Issue #545.

## What Changes

- Anything that would put another buffer in a window or change the window tree quits the view first and then runs in the restored layout: switching to or visiting a buffer (`C-x b`, `C-x C-f`, `C-x p f`, grep and link jumps, jumps back and forward, a session, terminal, list or recovered buffer shown), a buffer shown in another window (`display-buffer` answers), and splitting or deleting windows (`C-x 2`, `C-x 3`, `C-x 0`, `C-x 1`).
- Stays in the view: editing and saving the shown file, `C-x o`, the minibuffer and its previews, the view's own commands and its showing of a diff's buffers, showing the list itself, killing a buffer, an agent session replaced by its rollback in place, and switching workspaces (the view travels with the workspace).
- The audit of every command and answer that shows a buffer or changes the window tree is recorded in `docs/editor.md`.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `diff-view`: the view is left before anything else rearranges its windows.

## Impact

`crates/aiditor-core`: `visiting.rs` (`switch_to`, `show_buffer`), `frame.rs` (split, delete, `display_buffer`), `diff.rs` (quitting split out for reuse), `workspace_list.rs` (window ids that vanish with the view). `docs/editor.md` and `openspec/specs/diff-view`.
