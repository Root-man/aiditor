## MODIFIED Requirements

### Requirement: Answering a permission request
`C-c C-y` in a session buffer (`agent-allow-once`) SHALL send the multi-select question whose block point is on, as agent-questions describes; elsewhere it SHALL answer the oldest pending permission request with its first `allow_once` option, passing pending questions by. `C-c C-k` (`agent-reject-once`) SHALL answer the oldest pending request: a permission request with its first `reject_once` option, a question by skipping it, wherever point is. `RET` on an option line of any pending request SHALL answer it with that option; on any other line `RET` sends the prompt as before. The answer SHALL be the `selected` outcome with the option's id, and the request's block SHALL become a single line in `session-thought`, with no rule, that stays in the transcript: `✓ allowed once` for an `allow_once` option, `✓ allowed always` for `allow_always`, `✗ rejected` for `reject_once` and `✗ rejected always` for `reject_always`. With no pending permission request `C-c C-y`, and with no pending request `C-c C-k`, SHALL echo `No permission request is pending`; when the oldest has no option of the kind they SHALL echo `No allow_once option for <title>` (or `reject_once`) and answer nothing.

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

#### Scenario: Sending the multi-select question at point
- **WHEN** a permission request for `A` and then a multi-select question are pending, point is on the question's lines and `C-c C-y` is typed
- **THEN** the question is sent and `A` stays pending
