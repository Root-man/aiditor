# Proposal

## Why

The session's mode is read from ACP session modes, which ACP now supersedes with session config options: both adapters send a `mode`-category config option, and Codex's plan mode and model picking exist only as config options. The registry's `default_mode` is also still not applied, so a Claude session starts in the adapter's `default` instead of the configured `auto` (#305).

## What Changes

- The session's mode is the first `session/new` config option with category `mode`; an agent without one has no mode. Session modes are no longer read.
- `<agent> session ready (modes: ...)` lists that option's values; the mode line shows its current value and follows `config_option_update` instead of `current_mode_update`.
- After `session/new`, if the agent's `default_mode` is one of the option's values and not the current one, the core sends `session/set_config_option` through a new agent op, and the answer's options update the mode line. An unknown `default_mode`, or a failed set, is noted in `*Messages*` and leaves the mode alone.
- The fake agent advertises a `mode` select option (`auto` current, `plan`) instead of session modes, answers `session/set_config_option` and sends `config_option_update`; its `session/set_mode` handler goes. **BREAKING** for fake-agent scripts only in that it no longer advertises session modes.
- docs/editor.md records config options over modes, and why.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `agent-sessions`: the ready note lists the mode option's values; sessions start in `default_mode`, with unknown or failed values noted.
- `session-buffers`: the mode line reads the mode config option and follows `config_option_update`.
- `agent-processes`: a new op sends `session/set_config_option` and returns its answer to the core.
- `fake-agent`: advertises and sets a `mode` config option instead of session modes.

## Impact

`aiditor-core` (`agents.rs`, `lib.rs`: new `AgentOp` and `Msg` variants), `aiditor-services` (agent service, fake agent and their tests), `docs/editor.md`. No new dependencies; the types are in `agent-client-protocol-schema` 1.12 v1.
