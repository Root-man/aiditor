## ADDED Requirements

### Requirement: Forking a session
A spawn MAY name a session to fork and a message id to fork at. When the agent advertises `sessionCapabilities.fork`, it SHALL send `session/fork` of that session with `cwd` and the roots, and `_meta.jetbrains.air.fork = {"version": 1, "messageId": <id>}` when a message id is named, then load the session the fork answers as a load spawn does, so the replay precedes a started message with the new session id.

#### Scenario: A whole session is forked
- **WHEN** the test agent with fork and a two-update history is spawned to fork session `old` without a message id
- **THEN** the agent saw `session/fork` of `old` without a fork point, then `session/load` of the fork, and the core gets both updates, then a started message with the fork's session id

#### Scenario: A session is forked at a message
- **WHEN** the test agent with fork is spawned to fork session `old` at message `m1`
- **THEN** the agent saw `session/fork` of `old` with `_meta.jetbrains.air.fork` `{"version": 1, "messageId": "m1"}`

#### Scenario: An agent that can't fork
- **WHEN** the test agent without fork is spawned to fork a session
- **THEN** the core gets a failure started message with `agent can't fork sessions`
