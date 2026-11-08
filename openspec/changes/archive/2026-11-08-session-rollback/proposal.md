# Proposal

## Why

M6's last session feature is rollback (#350): after a turn went wrong, return the session to an earlier message and carry on from there, in the same buffer and window, without a second session lying around. Fork (#349) already cuts a session at a message; rollback is a fork that takes the source's place.

## What Changes

- New command `agent-rollback-session` (M-x) in a session buffer. It takes the message the `message-at-point` bar marks; with point on the prompt line or above the first message it echoes `No message at point`. As with fork, only agent messages with an id qualify: a prompt echoes `Roll back to an agent message`, an agent message without an id `No message id to roll back to`, a source whose `session/new` hasn't answered `<agent> is not ready`.
- It asks `Roll back to this message? Files stay as they are` (`no`, `yes`); the agents' file edits since that message stay on disk.
- On `yes` it forks at that message as `agent-fork-session` does (hidden buffer, own process, `session/fork` then `session/load`), echoing `Rolling back <agent> session...`.
- Once the fork's load answers, the fork takes the source's place: every window showing the source shows the fork's buffer, the source's buffer and log are killed, which stops its process, and the fork's buffer and log take their names. The next prompt goes to the fork. A failure echoes `<agent> rollback failed: <error>`, kills the fork's buffers and leaves the source as it was.
- **Changed from the issue (asked on #350):** the source's own process sends `session/delete` of it before it is stopped, when the agent advertises `sessionCapabilities.delete` (claude-agent-acp 0.97 and codex-acp 2.2.2 both do), so the source no longer lists in `agent-sessions`; open sessions buffers drop its row at once. An agent without it keeps the source, which lists again on `g`.
- The fake agent learns `"delete": true`.
- `docs/editor.md` records the decisions.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `agent-processes`: a spawn may roll back (a fork at its message); a delete op deletes a session, then stops its agent.
- `session-buffers`: `agent-rollback-session` replaces the session buffer's session with a fork at the message at point.
- `fake-agent`: the `delete` script key.

## Impact

- `crates/aiditor-core/src/lib.rs` (`Open::Rollback`), `agents.rs` (command, the swap, failure), `sessions.rs` (dropping the source's row), `builtins.rs`.
- `crates/aiditor-services/src/agents.rs` (`connect`), `bin/aiditor-fake-agent.rs`, `tests/agents.rs`.
- `crates/aiditor/src/editor.rs`: a test typed `M-x abc`, which the new command name matches.
- `docs/editor.md`.
