## ADDED Requirements

### Requirement: Rolling a session back to a message
`agent-rollback-session` (M-x) in a session buffer SHALL ask `Roll back to this message? Files stay as they are` (`no`, `yes`) and, on `yes`, fork the session at the message the `message-at-point` bar marks as `agent-fork-session` does, echoing `Rolling back <agent> session...`. When the fork's load answers, every window showing the source buffer SHALL show the fork's buffer, the source's buffer and log SHALL be killed, its agent deleting the source session before it stops, the source's rows SHALL leave the sessions buffers, and the fork's buffer and log SHALL take their names. Files are left as they are. Any other answer SHALL do nothing.

#### Scenario: Rolling back replaces the transcript
- **WHEN** point is on an agent message with id `m1` in session buffer `*claude*<work>`, `agent-rollback-session` runs, `yes` is answered and the fork's load replays the transcript up to `m1` and answers
- **THEN** the window shows a buffer named `*claude*<work>` holding that transcript, the next prompt goes to the fork's session, and the source's agent is sent a delete of its session

#### Scenario: The source leaves the sessions buffer
- **WHEN** `*Sessions*<work>` lists the source for `claude` and the same id for `codex`, and the rollback's load answers
- **THEN** only the `codex` row is left

#### Scenario: Declining the rollback
- **WHEN** `agent-rollback-session` runs on an agent message and `no` is answered
- **THEN** nothing is spawned and the session is unchanged

#### Scenario: A failed rollback leaves the source
- **WHEN** the rollback's fork fails with `agent can't fork sessions`
- **THEN** `claude rollback failed: agent can't fork sessions` is echoed, the fork's buffers are gone and the source buffer is shown and still sends prompts

### Requirement: What can't be rolled back
`agent-rollback-session` SHALL echo, ask nothing and spawn nothing when it can't roll back: `Not in an agent session buffer` outside one, `<agent> is not ready` before the source's session id is known, `No message at point` with point on the prompt line or before the first message, `Roll back to an agent message` when the marked message is a prompt, and `No message id to roll back to` when the marked agent message has no id.

#### Scenario: Point on the prompt line
- **WHEN** point is on a ready session's prompt line and `agent-rollback-session` runs
- **THEN** `No message at point` is echoed and nothing is asked or spawned

#### Scenario: Point on a prompt
- **WHEN** point is on a sent prompt and `agent-rollback-session` runs
- **THEN** `Roll back to an agent message` is echoed and nothing is asked or spawned
