## MODIFIED Requirements

### Requirement: Agent requests wait for the core's reply
Each request the agent sends to the client SHALL reach the core as a message with the agent id, the request id and the request. The request SHALL stay pending until the core replies to that agent and request id with a response or an error, which SHALL then be sent to the agent. A reply for an unknown agent or request id SHALL be ignored. The core SHALL answer `session/request_permission` for a ready session from its session buffer, SHALL answer it with the `cancelled` outcome for an agent whose session has ended or that has no session (its session buffer was killed), and SHALL reply to every other agent request, and to a permission request before the session is ready, with a "method not found" error.

#### Scenario: A request is rejected
- **WHEN** a session agent sends `terminal/create`
- **THEN** the core replies with a "method not found" error, which the service sends to the agent

#### Scenario: A permission request with no session is cancelled
- **WHEN** the test agent sends `session/request_permission` to a core that has no session for it
- **THEN** the core replies with the `cancelled` outcome, and the agent receives it

#### Scenario: A permission request is answered
- **WHEN** a session started with `agent-start` gets a permission request from the fake agent and `C-c C-y`, `C-c C-n` or `RET` on an option line answers it
- **THEN** the agent receives the `selected` outcome with that option's id
