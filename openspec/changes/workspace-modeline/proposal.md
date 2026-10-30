# Proposal

## Why

The project and workspace are visible only in the native window title, which macOS hides in fullscreen (#211). Repeating them in every buffer's mode line would duplicate them across splits.

## What Changes

- A frame-wide workspace modeline: one row above the window tree, outside the splits, showing `<project> › <workspace>` for the current workspace (design frames 2 and 11 of https://claude.ai/artifact/VyRd2AK1RfiVhuBx6nA1KP#page-dcb81aee8d83).
- The frame display carries both names, truncated to the frame's width so each keeps a visible part.
- The frontend draws it one line high in the editor font, project bold, separator dim, on the inactive mode line background.
- Buffer mode lines and the native window title stay as they are.

Out of scope (later work in the design): other workspaces, agent dots, the clock, switching controls.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `display-model`: the frame display carries the workspace modeline's project and workspace names, truncated to the frame width.
- `frontend-rendering`: the frontend draws the workspace modeline above every window.

## Impact

- `crates/aiditor-core/src/frame.rs`, `display.rs`: new `FrameDisplay` fields and the truncation.
- `crates/aiditor/src/editor.rs`: the bar row in `render`.
- Frame snapshots gain the new fields.
- `docs/editor.md` Workspaces › Display.
