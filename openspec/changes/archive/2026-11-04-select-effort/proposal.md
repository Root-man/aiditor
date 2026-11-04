# Proposal

## Why

The mode line shows the session's effort, but nothing lets the user change it. Issue #440 adds `agent-select-effort`, which does for the effort option what `agent-select-model` (#439) does for the model.

## What Changes

- Add `agent-select-effort` (M-x only): asks `Effort: ` over the values of the session's first select config option of category `thought_level`, by display name, groups flattened, the current one preselected and each value's description as its annotation; picking one sends `session/set_config_option`.
- The values are the ones the agent reported last, so after a model change the list offers the new model's efforts.
- Without an effort option it echoes `<agent> has no effort choice`; not ready or ended sessions echo as for the model. A refusal is noted as `<agent> could not set effort <value>: <reason>`.
- `agent-select-model` and `agent-select-effort` share one implementation.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `session-buffers`: effort selection.

## Impact

`aiditor-core` agents and builtins, core harness tests, `docs/editor.md`. No dependencies or protocol changes.
