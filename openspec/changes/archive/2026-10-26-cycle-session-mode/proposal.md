# Proposal

## Why

A session starts in the agent's default mode but nothing changes it afterwards, so leaving plan mode or switching to a more permissive one means restarting. Claude Code cycles its modes with shift-tab; issue #306 brings that key to the session buffer.

## What Changes

- New command `agent-cycle-mode`, bound to `S-TAB` in `agent-session-mode`: sends `session/set_config_option` setting the session's mode option to the value after its current one, wrapping around; the mode line follows the answer.
- It echoes `<agent> has no modes` for a session without a mode option, `<agent> is not ready` before `session/new` is answered and `<agent> session has ended` after an exit, sending nothing.
- It works while a turn runs, as ACP allows and Claude Code does.
- A refused set notes `<agent> could not set mode <value>: <reason>` with the value actually sent, cycled or default.
- docs/editor.md records the key and behaviour.

## Capabilities

### New Capabilities

### Modified Capabilities
- `session-buffers`: adds the `S-TAB` mode-cycling requirement.

## Impact

- `crates/aiditor-core`: `agents.rs` (command, the session keeps the latest config options and the value last sent), `builtins.rs`, `keymap.toml`.
- No service or protocol changes: `AgentOp::SetConfigOption` and `Msg::AgentConfigured` exist since #305.
- docs/editor.md.
