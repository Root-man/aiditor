## ADDED Requirements

### Requirement: Forking a session into a new session buffer
`agent-fork-session` (M-x) in a session buffer SHALL fork it as loading a listed session loads one: a hidden session buffer named after the source's title and a log, in the current workspace, spawning the source's agent to fork the source's session id. With point in the prompt area it SHALL fork without a message id; elsewhere at the id of the message the `message-at-point` bar marks. The source session SHALL be left as it was.

#### Scenario: Forking from the prompt area
- **WHEN** point is in the prompt area of a ready `claude` session titled `Fix it` in workspace `work` and `agent-fork-session` runs
- **THEN** the agent is spawned to fork the session without a message id, `*claude: Fix it*<work><2>` exists unshown, and `Forking claude session...` is echoed

#### Scenario: Forking at a message
- **WHEN** point is on a tool call under an agent message with id `m1` and `agent-fork-session` runs
- **THEN** the agent is spawned to fork the session at `m1`

#### Scenario: The fork shows once loaded
- **WHEN** the fork's load answers while the selected window still shows the source buffer
- **THEN** the selected window shows the fork's buffer with the replayed transcript, and the source buffer still sends prompts

### Requirement: What can't be forked
`agent-fork-session` SHALL echo and spawn nothing when it can't fork: `Not in an agent session buffer` outside one, `<agent> is not ready` before the source's session id is known, and `No message id to fork at` when point is in the transcript but no message is marked or the marked one has no id, as a prompt sent live has none.

#### Scenario: A prompt sent live
- **WHEN** point is on a prompt sent live and `agent-fork-session` runs
- **THEN** `No message id to fork at` is echoed and nothing is spawned

#### Scenario: A source not ready
- **WHEN** `agent-fork-session` runs before the session's `session/new` answers
- **THEN** `claude is not ready` is echoed and nothing is spawned

### Requirement: A failed fork opens nothing
When a fork's start fails, as with an agent that can't fork or an error answer, the echo area SHALL show `<agent> fork failed: <error>`, and the fork's session buffer and log SHALL be killed, so no buffer is left from it.

#### Scenario: An agent that can't fork
- **WHEN** the fork's started message fails with `agent can't fork sessions`
- **THEN** `claude fork failed: agent can't fork sessions` is echoed, neither of the fork's buffers exists and the selected window still shows the source
