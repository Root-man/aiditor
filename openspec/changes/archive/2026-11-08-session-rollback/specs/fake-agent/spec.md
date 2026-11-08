## ADDED Requirements

### Requirement: Deleting a session
With `"delete": true` the fake SHALL advertise `sessionCapabilities.delete`; otherwise it SHALL NOT. On `session/delete` it SHALL report `delete=<session id>` on stderr and answer with an empty response.

#### Scenario: A delete is reported
- **WHEN** the script has `"delete": true` and the client sends `session/delete` of `old`
- **THEN** the stderr has `delete=old`
