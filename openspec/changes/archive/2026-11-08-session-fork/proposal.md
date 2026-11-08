# Proposal

## Why

M6 needs a way to branch a conversation: try another direction from some point without losing the session you have (#349). ACP's unstable `session/fork` does it, and claude-agent-acp and codex-acp both advertise it.

## What Changes

- New command `agent-fork-session` (M-x) in a session buffer. With point in the prompt area it forks the whole session; with point in the transcript it forks up to the message at point (the one the `message-at-point` bar marks), passing `_meta.jetbrains.air.fork = { version: 1, messageId }`. Only agent messages are fork points: on a prompt it echoes `Fork at an agent message`, and on an agent message without an id `No message id to fork at`.
- The fork spawns its own agent process (one process per session) with the source's agent, in the current workspace's project, sends `initialize`, `session/fork` of the source's session id, then `session/load` of the new session, so the agent replays the forked history into the new buffer through the existing load path.
- The new buffer is created hidden and named like a loaded session (`*<agent>: <source title>*<<workspace>>`), and is shown once the load answers if the selected window still shows the source buffer, as loading a listed session does. The source session is untouched.
- An agent that doesn't advertise `sessionCapabilities.fork` fails with `agent can't fork sessions`; that or any other failure echoes the error and kills the hidden buffers, so nothing opens.
- The ACP crates' `unstable_session_fork` feature is enabled.
- The fake agent learns `"fork": true`: it advertises fork, reports the request on stderr and cuts its `history` at the fork point on the next load.
- `docs/editor.md` records the decisions.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `agent-processes`: a spawn may fork a session (fork, then load the fork).
- `session-buffers`: `agent-fork-session` forks the session at point into a new session buffer.
- `fake-agent`: the `fork` script key.

## Impact

- `crates/aiditor-core/src/lib.rs` (`AgentOp::Spawn`'s session field becomes an enum), `agents.rs` (command, start, failure), `builtins.rs`, `sessions.rs` (the loading lookup).
- `crates/aiditor-services/src/agents.rs` (`connect`), `bin/aiditor-fake-agent.rs`, `tests/agents.rs`, `tests/transcripts.rs`.
- `Cargo.toml` features of `agent-client-protocol` and `agent-client-protocol-schema`.
- `docs/editor.md`.
