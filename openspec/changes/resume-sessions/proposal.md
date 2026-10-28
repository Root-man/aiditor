# Proposal

## Why

`*Sessions*` lists every past session of the project, but `RET` on one that isn't open only says `Loading sessions isn't supported yet`. Continuing a session after a restart, or one started in the Claude Code CLI or the Codex app, is the point of the list (issue #103). Both adapters advertise `loadSession` and replay a session's history on `session/load` (checked against claude-agent-acp 0.95.0 and codex-acp 2.2.2).

## What Changes

- The agent service's spawn can load a session instead of opening one: after `initialize`, an agent advertising `loadSession` gets `session/load` with the session id and `cwd`; the replayed history streams to the core as session updates, then the started message answers with the session's config options. An agent without `loadSession` fails to start.
- `RET` on a sessions row that isn't open loads it in the current workspace: a new session buffer named after the listed title, with its stderr log, waiting for the project's environment like `agent-start`. The row shows `[starting]` until the load answers, and `RET` on it then switches to that buffer instead of loading twice.
- Replayed user messages show in the session buffer as typed prompts do (`> ` and the text, rendered as markdown); agent messages, thoughts, tool calls and the plan show as they already do.
- A loaded session keeps its own mode: the registry's `default_mode` applies to new sessions only.
- The fake agent can load: a script `history` list of updates makes it advertise `loadSession` and replay them on `session/load`.

## Capabilities

### New Capabilities

### Modified Capabilities
- `agent-processes`: spawning can load a session with `session/load`.
- `sessions-list`: `RET` on a row not open loads its session; a loading session is open in the editor and shows `[starting]`.
- `session-buffers`: a loaded session's buffer and replayed user messages.
- `fake-agent`: a `history` script key answers `session/load`.
- `agent-sessions`: the default mode applies to new sessions only.

## Impact

`crates/aiditor-core/src/lib.rs` (`AgentOp::Spawn` gains the session to load), `crates/aiditor-core/src/agents.rs` (start shared with loading, user message chunks, default mode), `crates/aiditor-core/src/sessions.rs` (`RET`), `crates/aiditor-services/src/agents.rs` (`session/load`), `crates/aiditor-services/src/bin/aiditor-fake-agent.rs`, tests, the bench and transcript test that build `Spawn`, and `docs/editor.md`. No new dependency or frontend change.
