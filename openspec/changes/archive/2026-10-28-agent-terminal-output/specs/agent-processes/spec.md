## MODIFIED Requirements

### Requirement: Spawning an agent and opening a session
An agent SHALL be spawned from an agent id chosen by the core, a command with its arguments, a working directory (the project's primary repo) and environment variables. Spawning SHALL return once the process has started, without waiting for the handshake. The agent's process SHALL inherit the editor's environment with the spawn's variables on top, applied in order so a later variable overrides an earlier one with the same name; the core lists the captured environment first, then the registry's `env`, then the project file's `env`. The command SHALL be looked up on the resulting `PATH`. Once started, the client SHALL send ACP `initialize` advertising neither file system capability nor the terminal capability, and `_meta.terminal_output: true`, then `session/new` with `cwd` set to the working directory and no MCP servers. The core SHALL get one started message for the agent id with the `session/new` response, holding the session id and the agent's config options, or with why the agent could not start: a command that cannot be found or run, a working directory that is not a directory, an error answer to either request, or the agent exiting before it answered.

#### Scenario: A session is opened
- **WHEN** the test agent is spawned in a temporary directory
- **THEN** the core gets a started message with the session id and config options the agent answered, and the agent saw `cwd` equal to that directory, `fs.readTextFile` and `fs.writeTextFile` false, `terminal` false and `terminal_output` true

#### Scenario: Later variables win
- **WHEN** the test agent is spawned with variables `A=1`, `A=2`
- **THEN** the agent sees `A=2`

#### Scenario: The command is found on the given PATH
- **WHEN** the test agent is spawned by its bare file name with `PATH` set to its directory
- **THEN** the core gets a started message with the session

#### Scenario: A missing program
- **WHEN** an agent is spawned with the command `/nonexistent/agent`
- **THEN** the core gets a failure started message for that id with the reason, and the service does not panic

#### Scenario: The agent exits before answering
- **WHEN** an agent is spawned with a command that exits at once without speaking ACP
- **THEN** the core gets a failure started message for that id, then its exit message
