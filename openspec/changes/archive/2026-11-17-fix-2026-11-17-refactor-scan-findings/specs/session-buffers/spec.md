## MODIFIED Requirements

### Requirement: Answering a permission request
`C-c C-y` in a session buffer (`agent-allow-once`) SHALL send the multi-select question, or the block of several questions, whose block point is on, as agent-questions describes; elsewhere it SHALL answer the oldest pending permission request with its first `allow_once` option, passing pending questions by. `C-c C-k` (`agent-reject-once`) SHALL answer the pending request whose block point is on: a permission request with its first `reject_once` option, a question by skipping it; elsewhere it SHALL answer the oldest pending request the same way. `RET` on an option line of any pending request SHALL answer it with that option; on any other line `RET` sends the prompt as before. The answer SHALL be the `selected` outcome with the option's id, and the request's block SHALL become a single line in `session-thought`, with no rule, that stays in the transcript: `✓ allowed once` for an `allow_once` option, `✓ allowed always` for `allow_always`, `✗ rejected` for `reject_once` and `✗ rejected always` for `reject_always`. With no pending permission request `C-c C-y`, and with no pending request `C-c C-k`, SHALL echo `No permission request is pending`; when the request they would answer has no option of the kind they SHALL echo `No allow_once option for <title>` (or `reject_once`) and answer nothing.

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

#### Scenario: Rejecting the request at point
- **WHEN** a question and then a permission request for `A` are pending, point is on `A`'s block and `C-c C-k` is typed
- **THEN** `A`'s `No` option is selected and the question stays pending

### Requirement: Rolling a session back to a message
`agent-rollback-session` (M-x) in a session buffer SHALL ask `Roll back to this message? Files stay as they are` (`no`, `yes`) and, on `yes`, fork the session at the message the `message-at-point` bar marks as `agent-fork-session` does, echoing `Rolling back <agent> session...`. When the fork's load answers, every window showing the source buffer SHALL show the fork's buffer, the source's buffer and log SHALL be killed, the fork's agent deleting the source session once the source's agent has stopped, the source's rows SHALL leave the sessions buffers, and the fork's buffer and log SHALL take their names. When the source session was pinned, the pin SHALL move to the fork's session and the pins file SHALL be rewritten. Files are left as they are. Any other answer SHALL do nothing.

#### Scenario: Rolling back replaces the transcript
- **WHEN** point is on an agent message with id `m1` in session buffer `*claude*<work>`, `agent-rollback-session` runs, `yes` is answered and the fork's load replays the transcript up to `m1` and answers
- **THEN** the window shows a buffer named `*claude*<work>` holding that transcript, the next prompt goes to the fork's session, and the source's agent is stopped with the fork's agent deleting its session

#### Scenario: The source leaves the sessions buffer
- **WHEN** `*Sessions*<work>` lists the source for `claude` and the same id for `codex`, and the rollback's load answers
- **THEN** only the `codex` row is left

#### Scenario: A pinned session stays pinned
- **WHEN** the source session is pinned and the rollback's load answers
- **THEN** `sessions.toml` pins the fork's session instead of the source's, and the fork lists under Pinned

#### Scenario: Declining the rollback
- **WHEN** `agent-rollback-session` runs on an agent message and `no` is answered
- **THEN** nothing is spawned and the session is unchanged

#### Scenario: A failed rollback leaves the source
- **WHEN** the rollback's fork fails with `agent can't fork sessions`
- **THEN** `claude rollback failed: agent can't fork sessions` is echoed, the fork's buffers are gone and the source buffer is shown and still sends prompts
