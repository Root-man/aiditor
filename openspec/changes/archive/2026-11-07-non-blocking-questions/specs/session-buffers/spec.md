## MODIFIED Requirements

### Requirement: Session mode lines show lifecycle status
The right group SHALL show `● working` in `status-running` while a prompt awaits its answer, `● awaiting permission` in `status-waiting` while any permission request is pending, `● awaiting answer` in `status-waiting` while a blocking question is pending and no permission request is, or `● exited` in `status-failed` after exit or failed startup, with exited taking precedence over awaiting permission, which takes precedence over awaiting answer, which takes precedence over working. A non-blocking question SHALL NOT change the status; while any is pending, the right group SHALL show `<n> question` (`<n> questions` for more than one), counting them, in `status-waiting` after the status. A live session without a running prompt or pending request SHALL omit status. Answering one of several requests SHALL keep awaiting-permission status until all are answered.

#### Scenario: Turn and permission lifecycle
- **GIVEN** a live idle session
- **WHEN** a prompt starts, two permissions arrive, one is answered, then the other is answered, then the prompt ends
- **THEN** the status progresses through working, awaiting permission, awaiting permission, working, then no status

#### Scenario: A non-blocking question during a turn
- **GIVEN** a running turn
- **WHEN** a non-blocking question arrives
- **THEN** the right group reads `● working`, then `1 question`

#### Scenario: Session ends
- **GIVEN** a running session with a pending request or a starting session
- **WHEN** the agent exits or fails to start
- **THEN** its status is `● exited`
