## MODIFIED Requirements

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
