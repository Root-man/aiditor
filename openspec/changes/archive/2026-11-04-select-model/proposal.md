# Proposal

## Why

Codex offers its model only as an ACP config option, and the mode line already shows the model, but nothing lets the user change it. Issue #439 adds an editor command that picks the session's model through `session/set_config_option`, as `S-TAB` sets the mode.

## What Changes

- Add `agent-select-model` (M-x only): asks `Model: ` over the session's model option's values by display name, groups flattened, the current one preselected, each value's description as its annotation; picking one sends `session/set_config_option` with the option's id and the value's id.
- The mode line follows the answer and later `config_option_update`s, as it already does.
- A refused set is noted in `*Messages*` naming what was sent (`could not set model <value>`), the same note `S-TAB`'s refusals get.
- Without a model option, before `session/new` is answered or after the agent exits, it echoes and sends nothing.
- The fake agent advertises a `model` option and accepts setting it.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `session-buffers`: model selection.
- `fake-agent`: a `model` config option.

## Impact

`aiditor-core` agents and builtins, the fake agent binary, core harness and services tests, `docs/editor.md`. No dependencies or protocol changes.
