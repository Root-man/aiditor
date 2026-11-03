# Proposal

## Why

Long pasted logs overwhelm the session prompt. Issue #372 calls for a compact paste chip while preserving all input for the agent.

## What Changes

- Add `paste_fold_lines`, default 20, and fold prompt yanks strictly above it.
- Draw `≡ Pasted N lines` in the attachment face; TAB toggles a four-line preview with `… N more lines`, as confirmed by the user from frame 6b.
- Keep the original text in the rope and send all of it. Fold the sent transcript copy even when the preview was open.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `session-buffers`: folded pastes and TAB previews.
- `settings`: configurable paste line threshold.
- `display-model`: replacement text may include a line break for a paste header.

## Impact

Core settings, yank/yank-pop, session rendering and sending, display projection, harness tests, and docs/editor.md. No dependencies or ACP protocol changes.
