# Proposal

## Why

Starting agent sessions (#94's next tasks) needs to know how to launch each agent. Project files already name a `default_agent`, but nothing checks it because no registry exists yet (#278).

## What Changes

- An `[agents.<name>]` table in `settings.toml` with `command`, `args`, `env` and `default_mode`, merged over the bundled settings like any other table, so a user entry overrides single keys of a bundled agent or adds a new agent.
- The bundled settings define `claude`, running `claude-agent-acp` in the `auto` mode.
- A bad agent entry (unknown key, wrong type, empty command, bad `env` variable) rejects the settings file like any other settings error.
- A loaded project file whose `default_agent` names no registered agent is reported as `project <file>: <reason>`; the project still loads and keeps the value.
- `project-set-default-agent` completes over the registered agents and refuses a name that is not one.
- `docs/editor.md` records the decision and drops the "not yet" note about `default_agent`.

## Capabilities

### New Capabilities
- `agent-registry`: the agents the editor can start, defined in settings with bundled defaults.

### Modified Capabilities
- `projects`: `default_agent` is checked against the registry when files load and when it is set by command.

## Impact

`aiditor-core`: `settings.rs`, the bundled `settings.toml`, project reconciliation in `project_commands.rs`, and the `project-set-default-agent` completion in `builtins.rs`. No new dependencies. Spawning agents is a later task.
