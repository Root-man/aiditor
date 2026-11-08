## ADDED Requirements

### Requirement: Rolling a session back
A spawn MAY roll a session back to a message id; it SHALL fork that session at the message and load the fork as a fork spawn does.

#### Scenario: A rollback is a fork at its message
- **WHEN** the test agent with fork is spawned to roll session `old` back to message `m1`
- **THEN** the agent saw `session/fork` of `old` at `m1` and `session/load` of the fork, no `session/delete`, and the core gets a started message with the fork's session id

### Requirement: Deleting a stopped agent's session
A delete SHALL stop its source agent as a kill does and, once the source's process group is gone, send `session/delete` of the session through the other agent it names when that agent advertises `sessionCapabilities.delete`, and nothing otherwise. A failed delete SHALL only be logged.

#### Scenario: An agent that can delete
- **WHEN** a delete of `fake-session` names a running source and the test agent with delete
- **THEN** the source exits and the test agent saw `session/delete` of `fake-session`

#### Scenario: An agent that can't delete
- **WHEN** a delete names a running source and the test agent without delete
- **THEN** the source exits and the test agent saw no `session/delete`
