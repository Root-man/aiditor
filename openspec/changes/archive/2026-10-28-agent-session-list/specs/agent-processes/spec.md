## ADDED Requirements

### Requirement: Listing an agent's sessions
A list request SHALL carry a request number chosen by the core, a command with its arguments, a working directory (the project's primary repo) and environment variables, and SHALL return at once. The agent SHALL be spawned as for a session, in its own process group, with its stderr discarded. The client SHALL send ACP `initialize` and, when the agent advertises `sessionCapabilities.list`, `session/list` with `cwd` set to the working directory, then `session/list` again with each `nextCursor` until an answer has none or 50 pages were asked. The client SHALL then send SIGTERM to the process group, and SIGKILL 100 ms later unless it exited. The core SHALL get one listed message with the request number and every page's sessions in order, or why it could not list: a command that cannot be found or run, a working directory that is not a directory, an agent without `sessionCapabilities.list`, an error answer, or the agent exiting before it answered.

#### Scenario: Every page is listed
- **WHEN** the test agent with two pages of sessions in its working directory and one session elsewhere is listed in that directory
- **THEN** the core gets a listed message with the request number and the sessions of both pages in order, without the other one, and within seconds the agent's process no longer exists

#### Scenario: No list capability
- **WHEN** the test agent without sessions is listed
- **THEN** the core gets a listed failure saying the agent can't list sessions

#### Scenario: A missing program
- **WHEN** a list request has the command `/nonexistent/agent`
- **THEN** the core gets a listed failure with the reason

#### Scenario: The agent exits before answering
- **WHEN** a list request has a command that exits at once without speaking ACP
- **THEN** the core gets a listed failure

## MODIFIED Requirements

### Requirement: No agents left behind on quit
When the app quits, every live agent and every agent still listing sessions SHALL be killed before the process exits, waiting once for the 100 ms grace before SIGKILL.

#### Scenario: Quitting kills every agent
- **WHEN** two test agents are running, one ignoring SIGTERM, and all agents are killed for quitting
- **THEN** it returns after the grace, and within seconds neither agent's process exists
