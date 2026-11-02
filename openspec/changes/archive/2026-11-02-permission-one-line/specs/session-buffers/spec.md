## MODIFIED Requirements

### Requirement: Answering a permission request
`C-c C-y` in a session buffer (`agent-allow-once`) SHALL answer the oldest pending request with its first `allow_once` option, and `C-c C-k` (`agent-reject-once`) with its first `reject_once` option, wherever point is. `RET` on an option line of any pending request SHALL answer it with that option; on any other line `RET` sends the prompt as before. The answer SHALL be the `selected` outcome with the option's id, and the request's block SHALL become a single line in `session-thought`, with no rule, that stays in the transcript: `✓ allowed once` for an `allow_once` option, `✓ allowed always` for `allow_always`, `✗ rejected` for `reject_once` and `✗ rejected always` for `reject_always`. With no pending request `C-c C-y` and `C-c C-k` SHALL echo `No permission request is pending`; when the oldest has no option of the kind they SHALL echo `No allow_once option for <title>` (or `reject_once`) and answer nothing.

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

### Requirement: Cancelling ends pending permission requests
When `C-c C-c` sends `session/cancel`, or the agent's answer to the prompt ends a turn, every pending permission request of the session SHALL be answered with the `cancelled` outcome. When the agent exits, pending requests are dropped unanswered. Each way, each block SHALL become the line `✗ cancelled` in `session-thought`, with no rule. A permission request that arrives after the session ended, or after its session buffer was killed, SHALL be answered `cancelled` and SHALL NOT be shown.

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
