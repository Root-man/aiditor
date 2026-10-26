# agent-registry Specification

## Purpose
The agents the editor can start: each agent's command, arguments, environment and starting mode, defined in settings with bundled defaults the user can override or extend.

## Requirements

### Requirement: Agents are defined in settings
The settings SHALL hold an `agents` table keyed by agent name, each entry having a `command`, an optional `args` list of strings (default none), an optional `env` table of string variables (default none) and an optional `default_mode` string. The bundled settings SHALL define an agent named `claude` whose command is `claude-agent-acp`, whose default mode is `auto` and whose `env` sets `CLAUDE_CODE_ENABLE_TASKS` and `CLAUDE_CODE_ENABLE_TODO_TOOLS` to `1`, which turn on the tools Claude sends its plan with. A user's `[agents.<name>]` table SHALL be merged over the bundled one key by key, as other settings tables are, and a name the bundled settings do not define SHALL add an agent.

#### Scenario: Bundled claude agent
- **WHEN** no user settings file exists
- **THEN** the registry holds `claude` with command `claude-agent-acp`, no arguments, default mode `auto` and the two plan tool variables set to `1`

#### Scenario: User overrides one key of a bundled agent
- **WHEN** the user file sets `[agents.claude]` `command = "/opt/bin/claude-agent-acp"`
- **THEN** `claude` uses that command and keeps the bundled default mode and environment

#### Scenario: User adds an agent
- **WHEN** the user file defines `[agents.codex]` with `command = "codex-acp"`
- **THEN** the registry holds both `claude` and `codex`

### Requirement: Bad agent entries reject the settings
An agent entry with a key the editor does not know, a value of the wrong type, a missing or empty command, or an `env` variable whose name is empty or holds `=` or NUL or whose value holds NUL SHALL reject the settings file as any invalid setting does: the error names the agent and the problem, and the previous settings stay in effect.

#### Scenario: Unknown key in an agent
- **WHEN** the user file sets `[agents.claude]` `comand = "x"` and the settings are reloaded
- **THEN** an error naming `comand` is shown and the previous registry stays in effect

#### Scenario: Empty command
- **WHEN** the user file defines `[agents.mine]` with `command = ""`
- **THEN** an error naming `mine` is shown and the previous settings stay in effect

#### Scenario: Bad env variable
- **WHEN** the user file sets `[agents.claude.env]` `"A=B" = "c"`
- **THEN** an error naming the agent and the variable is shown and the previous settings stay in effect
