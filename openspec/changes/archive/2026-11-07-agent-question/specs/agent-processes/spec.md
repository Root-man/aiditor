# Spec Delta

## MODIFIED Requirements

### Requirement: Spawning an agent and opening a session
An agent SHALL be spawned from an agent id chosen by the core, a command with its arguments, a working directory (the project's primary repo) and environment variables. Spawning SHALL return once the process has started, without waiting for the handshake. The agent's process SHALL inherit the editor's environment with the spawn's variables on top, applied in order so a later variable overrides an earlier one with the same name; the core lists the captured environment first, then the registry's `env`, then the project file's `env`. The command SHALL be looked up on the resulting `PATH`. Once started, the client SHALL send ACP `initialize` advertising neither file system capability nor the terminal capability, `elicitation.form` and not `elicitation.url`, and `_meta.terminal_output: true`, then `session/new` with `cwd` set to the working directory and no MCP servers. The core SHALL get one started message for the agent id with the `session/new` response, holding the session id and the agent's config options, or with why the agent could not start: a command that cannot be found or run, a working directory that is not a directory, an error answer to either request, or the agent exiting before it answered.

#### Scenario: A session is opened
- **WHEN** the test agent is spawned in a temporary directory
- **THEN** the core gets a started message with the session id and config options the agent answered, and the agent saw `cwd` equal to that directory, `fs.readTextFile` and `fs.writeTextFile` false, `terminal` false, `terminal_output` true, `elicitation.form` true and `elicitation.url` false

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

### Requirement: Agent requests wait for the core's reply
Each request the agent sends to the client SHALL reach the core as a message with the agent id, the request id and the request. The request SHALL stay pending until the core replies to that agent and request id with a response or an error, which SHALL then be sent to the agent. A reply for an unknown agent or request id SHALL be ignored. The core SHALL answer `session/request_permission` and `elicitation/create` for a ready session from its session buffer, SHALL answer them with the `cancelled` outcome and the `cancel` action for an agent whose session has ended or that has no session (its session buffer was killed), and SHALL reply to every other agent request, and to either before the session is ready, with a "method not found" error. When the agent sends `$/cancel_request` for a pending request, the service SHALL answer that request with the request-cancelled error, drop it from the pending requests so a later reply from the core is ignored, and tell the core with a withdrawn message holding the agent id and request id.

#### Scenario: A request is rejected
- **WHEN** a session agent sends `terminal/create`
- **THEN** the core replies with a "method not found" error, which the service sends to the agent

#### Scenario: A permission request with no session is cancelled
- **WHEN** the test agent sends `session/request_permission` to a core that has no session for it
- **THEN** the core replies with the `cancelled` outcome, and the agent receives it

#### Scenario: A permission request is answered
- **WHEN** a session started with `agent-start` gets a permission request from the fake agent and `C-c C-y`, `C-c C-k` or `RET` on an option line answers it
- **THEN** the agent receives the `selected` outcome with that option's id

#### Scenario: A question is answered
- **WHEN** a session started with `agent-start` gets a question from the fake agent and a digit, an own answer, `C-c C-k` or `C-c C-c` answers it
- **THEN** the agent receives `accept` with that content, `decline` or `cancel`

#### Scenario: A withdrawn request
- **WHEN** the fake agent sends a question and withdraws it with `$/cancel_request`
- **THEN** the agent receives the request-cancelled error and the core gets the withdrawn message
