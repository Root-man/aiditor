# Proposal

## Why

Sessions start in the project's primary repo only, so the agent can't see or edit the project's other roots and doesn't know what each root is for. ACP's `additionalDirectories` and a first-prompt preamble fix both (issue #329, docs/editor.md "ACP integration and session management").

## What Changes

- Starting or loading a session passes the project's other roots as `additionalDirectories` on `session/new` and `session/load` when the agent advertises `sessionCapabilities.additionalDirectories`; otherwise the session starts without them and `*Messages*` notes `<agent> can't take additional directories`.
- A new session's first prompt carries a roots preamble (each root's path and role line, the primary marked) as a text block before the user's text. It is not shown in the session buffer, and is skipped when there is nothing to list. Loaded sessions get none.
- Roots are taken when the session starts or loads; changes while it runs apply from the next start or load.
- The fake agent can advertise the capability and reports the directories it receives.
- Known limit: a loaded session's replayed history shows the preamble as part of its first user message.

## Capabilities

### New Capabilities

### Modified Capabilities
- `agent-processes`: spawning and loading send the other roots when the agent can take them, and report when it can't.
- `agent-sessions`: sessions start with the project's other roots, note an agent that can't take them, and send the roots preamble on a new session's first prompt.
- `fake-agent`: a script key advertises additional directories; the fake reports the ones it receives.

## Impact

`aiditor-core` (`AgentOp::Spawn`, a new `Msg`, `agents.rs` session start and prompt), `aiditor-services` (`agents.rs` connect, the fake agent binary, tests), docs/editor.md.
