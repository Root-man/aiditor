# Proposal

## Why

The frame shows one workspace at a time and nothing on screen says which one, or which project it is bound to; `workspace-list` was the stopgap (#65). Rather than a separate strip, the window title, which already names the current buffer, names the workspace and project too.

## What Changes

- The window title becomes `<buffer>[ *] | <workspace> | <project>`, for the current workspace, following switches and renames.
- `docs/editor.md` records that the "strip along the top" is the window title, and drops "until the strip lands" from `workspace-list`.

## Capabilities

### New Capabilities

### Modified Capabilities
- `display-model`: the frame display's title names the current workspace and its project after the buffer.

## Impact

`crates/aiditor-core/src/frame.rs` (title format and its test), `display.rs` (doc comment), `docs/editor.md`. The frontend already sets the OS window title from the frame display; insta snapshots carrying `title` change.
