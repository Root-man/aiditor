# Proposal

## Why

Project references only offer indexed paths. Issue #371 needs an explicit way to attach any file, image or directory on disk, including paths outside the project.

## What Changes

- Add `M-x agent-attach-file`, reading `Attach: ` with find-file's filesystem completion and annotations while the session stays visible.
- Keep `RET` directory navigation and add picker-local `C-RET` to attach the selected path as it is.
- Insert the existing chip at prompt point, preserving surrounding text and the existing chip send behavior.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `session-buffers`: attach arbitrary disk paths through a filesystem picker.

## Impact

Core command registration, minibuffer acceptance, command-local keymap, session insertion, core and GPUI tests, and `docs/editor.md`. No dependencies or ACP changes.
