## MODIFIED Requirements

### Requirement: Deleting a session
With `"delete": true` the fake SHALL advertise `sessionCapabilities.delete`; otherwise it SHALL NOT. On `session/delete` it SHALL report `delete=<session id>` on stderr, answer with an empty response, and leave the session out of its later `session/list` answers.

#### Scenario: A delete is reported
- **WHEN** the script has `"delete": true` and the client sends `session/delete` of `old`
- **THEN** the stderr has `delete=old`

#### Scenario: A deleted session no longer lists
- **WHEN** the script lists `old` and `new` and the client sends `session/delete` of `old`, then `session/list`
- **THEN** the answer has only `new`
