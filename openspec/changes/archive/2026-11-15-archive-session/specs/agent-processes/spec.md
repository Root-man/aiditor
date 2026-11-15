## ADDED Requirements

### Requirement: Deleting a session before listing
A list request MAY name a session to delete and a running agent to stop. The named agent SHALL be stopped as a kill stops it. After `initialize`, and once the stopped agent's process group is gone, the client SHALL send `session/delete` of the session, then list as any list request does. The list SHALL fail, with the session and the reason, when the agent doesn't advertise `sessionCapabilities.delete` or answers the delete with an error.

#### Scenario: The deleted session no longer lists
- **WHEN** the test agent with sessions `a` and `b` that can delete is listed deleting `a`
- **THEN** the core gets a listed message with only `b`

#### Scenario: An agent that can't delete
- **WHEN** the test agent with sessions that can't delete is listed deleting `a`
- **THEN** the core gets a listed failure saying the agent can't delete sessions

#### Scenario: The open session's agent is stopped first
- **WHEN** a running test agent is named to stop in a list request deleting its session
- **THEN** that agent exits and the core gets the listed message
