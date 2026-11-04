# Proposal

## Why

The workspace list keeps its keys in a buffer header that consumes space and omits folding. Issue #425 moves the current actions into the echo line, matching frames 13–13d and leaving the list for project/workspace rows.

## What Changes

- Remove the workspace list's header and leading blank row.
- Show `n/p move   RET switch   TAB fold   q quit` while the selected window shows the list, unless a message or minibuffer takes precedence.
- Build the hints from one action table, drawing keys bold in the existing workspace-current foreground and labels dimmed.
- Wrap complete hints into a second echo row when needed; the windows yield that height.
- Update documentation and automate selection, message lifetime, wrapping and height acceptance.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `workspaces`: contextual workspace-list hints and a headerless list.

## Impact

Workspace snapshot rendering and affected fixtures; the core frame display; frontend echo layout and GPUI tests; docs/editor.md. Existing list actions, snapshot/fold behavior and key bindings remain the baseline from workspace-list-project-rows, workspace-list-columns and workspace-list-folds. Narrow list-column fitting (#426) is outside this change. No dependencies or persistence changes.
