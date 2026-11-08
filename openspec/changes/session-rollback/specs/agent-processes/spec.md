## ADDED Requirements

### Requirement: Rolling a session back
A spawn MAY roll a session back to a message id. It SHALL fork that session at the message and load the fork as a fork spawn does; once the load answers, and before the started message, it SHALL send `session/delete` of the source session when the agent advertises `sessionCapabilities.delete`, and send nothing more otherwise. A failed delete SHALL NOT fail the start.

#### Scenario: A rollback deletes the source
- **WHEN** the test agent with fork and delete is spawned to roll session `old` back to message `m1`
- **THEN** the agent saw `session/fork` of `old` at `m1`, `session/load` of the fork, then `session/delete` of `old`, and the core gets a started message with the fork's session id

#### Scenario: An agent that can't delete keeps the source
- **WHEN** the test agent with fork but without delete is spawned to roll session `old` back to `m1`
- **THEN** the core gets a started message with the fork's session id and the agent saw no `session/delete`
