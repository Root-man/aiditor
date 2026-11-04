# Proposal

## Why

#424 makes the workspace list manageable as projects and workspaces grow. Folding keeps task context available while hiding its buffer inventory, following approved frames 13–13d.

## What Changes

- TAB toggles project/workspace children while retaining point on the row; rows without children have no marker and do nothing.
- Projects start open; workspaces start folded except the current one. Existing folds survive refresh; dismissal resets them.
- Use indentation and session-style fold markers, blank rows after open projects, and folded-project count/state summaries.
- Navigation, activation and position follow visible selectable rows; inventory totals retain all snapshot projects/workspaces.
- Keep #425 echo hints/header removal and #426 narrow fitting out of scope.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `workspaces`: foldable workspace-list snapshot and visible-row navigation.

## Impact

Core workspace-list snapshot and buffer presentation state, command/key binding, mode-line counts, harness/GPUI acceptance checks and docs/editor.md. No dependency or persistence changes. The verified local #422/#423 deltas are the baseline; those changes remain unarchived.
