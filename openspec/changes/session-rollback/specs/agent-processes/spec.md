## ADDED Requirements

### Requirement: Rolling a session back
A spawn MAY roll a session back to a message id; it SHALL fork that session at the message and load the fork as a fork spawn does.

#### Scenario: A rollback is a fork at its message
- **WHEN** the test agent with fork is spawned to roll session `old` back to message `m1`
- **THEN** the agent saw `session/fork` of `old` at `m1` and `session/load` of the fork, no `session/delete`, and the core gets a started message with the fork's session id

### Requirement: Deleting a session before stopping
A delete of a running agent's session SHALL send `session/delete` of it when the agent advertises `sessionCapabilities.delete`, then stop the agent as a kill does once the agent answers, or after 5 seconds without an answer; an agent without the capability SHALL be stopped at once. A failed delete SHALL only be logged.

#### Scenario: An agent that can delete
- **WHEN** a delete of `fake-session` is sent to the test agent with delete
- **THEN** the agent saw `session/delete` of `fake-session` and exits

#### Scenario: An agent that can't delete
- **WHEN** a delete is sent to the test agent without delete
- **THEN** it exits without seeing `session/delete`
