# Proposal

## Why

Review of the workspace list found distracting internal buffers, lost session label colours, refresh that moves point, and weak separation between folded projects. Refine the pending epic stack without changing split-window behavior.

## What Changes

- List and count owned files, terminals and registered agent sessions only.
- Retain session model/effort item-quality faces from the shared session helper.
- Bind `g` to refresh, show its hint, and preserve selected row identity in every window sharing the list, including informational buffer rows.
- Put one real horizontal separator between every pair of adjacent projects, independent of folds and emptiness.
- Keep each fitted snapshot row on one visual row at every font/window width, even when ordinary buffers wrap.
- Update the workspace-list contracts and automate the review cases.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `workspaces`: inventory eligibility, location faces, identity-preserving refresh, echo hints and project separators.

## Impact

Core workspace-list snapshot/display, keymap, affected tests and docs/editor.md. This change modifies the contracts introduced by the five pending workspace-list changes and must be synced/archived after those five. No dependencies, full-frame behavior or GitHub changes.
