# Spec Delta

## MODIFIED Requirements

### Requirement: Answering a permission request
`C-c C-y` in a session buffer (`agent-allow-once`) SHALL answer the oldest pending permission request with its first `allow_once` option, passing pending questions by, and `C-c C-k` (`agent-reject-once`) SHALL answer the oldest pending request: a permission request with its first `reject_once` option, a question by skipping it, wherever point is. `RET` on an option line of any pending request SHALL answer it with that option; on any other line `RET` sends the prompt as before. The answer SHALL be the `selected` outcome with the option's id, and the request's block SHALL become a single line in `session-thought`, with no rule, that stays in the transcript: `✓ allowed once` for an `allow_once` option, `✓ allowed always` for `allow_always`, `✗ rejected` for `reject_once` and `✗ rejected always` for `reject_always`. With no pending permission request `C-c C-y`, and with no pending request `C-c C-k`, SHALL echo `No permission request is pending`; when the oldest has no option of the kind they SHALL echo `No allow_once option for <title>` (or `reject_once`) and answer nothing.

#### Scenario: Allowing
- **WHEN** a request for `Edit a.txt` is pending and `C-c C-y` is typed in the prompt area
- **THEN** the `Yes` option is selected and the block reads `✓ allowed once`

#### Scenario: Rejecting
- **WHEN** the same request is pending and `C-c C-k` is typed
- **THEN** the `No` option is selected and the block reads `✗ rejected`

#### Scenario: Allowing always
- **WHEN** point is on the line of an `allow_always` option and `RET` is typed
- **THEN** that option is selected and the block reads `✓ allowed always`

#### Scenario: Rejecting always
- **WHEN** point is on the line of a `reject_always` option and `RET` is typed
- **THEN** that option is selected and the block reads `✗ rejected always`

#### Scenario: RET on an option
- **WHEN** point is on the line `2. No (reject_once)` and `RET` is typed
- **THEN** the `No` option is selected and no prompt is sent

#### Scenario: Oldest first
- **WHEN** requests for `A` and then `B` are pending and `C-c C-y` is typed twice
- **THEN** `A` is answered first, then `B`

#### Scenario: Nothing to answer
- **WHEN** no request is pending, or the oldest has no `allow_once` option, and `C-c C-y` is typed
- **THEN** `No permission request is pending`, or `No allow_once option for <title>`, is echoed and nothing is answered

#### Scenario: Allowing passes a question by
- **WHEN** a question and then a permission request for `A` are pending and `C-c C-y` is typed
- **THEN** `A` is answered and the question stays pending

### Requirement: Cancelling ends pending permission requests
When `C-c C-c` sends `session/cancel`, the agent's answer to the prompt ends a turn, or the session buffer is killed, every pending permission request of the session SHALL be answered with the `cancelled` outcome. When the agent exits, pending requests are dropped unanswered. Each way, each block SHALL become the line `✗ cancelled` in `session-thought`, with no rule. A permission request that arrives after the session ended, or after its session buffer was killed, SHALL be answered `cancelled` and SHALL NOT be shown.

#### Scenario: Cancelling with a pending request
- **WHEN** a request for `Edit a.txt` is pending and `C-c C-c` is typed
- **THEN** it is answered `cancelled`, `session/cancel` is sent and the block reads `✗ cancelled`

#### Scenario: The turn ends with a pending request
- **WHEN** a request for `Edit a.txt` is pending and the agent answers the prompt, with any stop reason or an error
- **THEN** it is answered `cancelled` and the block reads `✗ cancelled`

#### Scenario: A request after the session ended
- **WHEN** the agent has exited and a permission request from it arrives
- **THEN** it is answered `cancelled` and the session buffer is unchanged

#### Scenario: A request after the session buffer was killed
- **WHEN** the session buffer was killed and a permission request from its agent arrives before the agent exits
- **THEN** it is answered `cancelled`

#### Scenario: Killing the buffer with a pending request
- **WHEN** a request for `Edit a.txt` is pending and the session buffer is killed
- **THEN** it is answered `cancelled` before the agent is stopped

### Requirement: Session mode lines show lifecycle status
The right group SHALL show `● working` in `status-running` while a prompt awaits its answer, `● awaiting permission` in `status-waiting` while any permission request is pending, `● awaiting answer` in `status-waiting` while a question is pending and no permission request is, or `● exited` in `status-failed` after exit or failed startup, with exited taking precedence over awaiting permission, which takes precedence over awaiting answer, which takes precedence over working. A live session without a running prompt or pending request SHALL omit status. Answering one of several requests SHALL keep awaiting-permission status until all are answered.

#### Scenario: Turn and permission lifecycle
- **GIVEN** a live idle session
- **WHEN** a prompt starts, two permissions arrive, one is answered, then the other is answered, then the prompt ends
- **THEN** the status progresses through working, awaiting permission, awaiting permission, working, then no status

#### Scenario: Session ends
- **GIVEN** a running session with a pending request or a starting session
- **WHEN** the agent exits or fails to start
- **THEN** its status is `● exited`
