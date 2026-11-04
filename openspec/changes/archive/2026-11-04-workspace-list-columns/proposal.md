# Proposal

## Why

The workspace list's plain tree omits the locations and states needed to choose a task. Issue #423 brings its snapshot rows to the approved frame 13/13b columns and faces.

## What Changes

- Show name, where and state columns for projects, workspaces and owned buffers.
- Include workspace buffer/unsaved counts and the workspace modeline's most urgent agent state.
- Reuse file directories, terminal cwd/command/exit state, and session model/effort/activity.
- Omit owned-buffer workspace suffixes, escape control characters, and put point on selectable names.
- Use existing named faces in both themes and document the behavior.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `workspaces`: add snapshot row metadata, columns, faces and name selection on top of the unarchived #422 project-row contract.

## Impact

Core workspace-list snapshot generation and display, shared mode-line metadata where needed, core harness tests, and docs/editor.md. No dependencies or storage changes. Folding, echo hints and narrow-layout behavior remain in #424–426.
