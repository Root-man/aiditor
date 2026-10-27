## MODIFIED Requirements

### Requirement: Agent requests wait for the core's reply
Each request the agent sends to the client SHALL reach the core as a message with the agent id, the request id and the request. The request SHALL stay pending until the core replies to that agent and request id with a response or an error, which SHALL then be sent to the agent. A reply for an unknown agent or request id SHALL be ignored. The core SHALL answer `session/request_permission` for a ready session from its session buffer, and SHALL reply to every other agent request, and to a permission request for an agent without a ready session, with a "method not found" error.

#### Scenario: A request is rejected
- **WHEN** the test agent sends `session/request_permission` to a core that has no session for it
- **THEN** the core replies with a "method not found" error, and the agent receives that error

#### Scenario: A permission request is answered
- **WHEN** a session started with `agent-start` gets a permission request from the fake agent and `C-c C-y`, `C-c C-n` or `RET` on an option line answers it
- **THEN** the agent receives the `selected` outcome with that option's id
