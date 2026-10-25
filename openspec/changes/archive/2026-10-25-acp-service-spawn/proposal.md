# Proposal

## Why

M3 needs agent sessions, and every later agent feature (starting a session from the editor, the session buffer, modes, permissions) talks to an agent process through one service. The threading spike (#95) settled that this service runs on the existing Tokio runtime; this change builds it: spawn an ACP agent, initialize it, open a session and report back to the core (#279).

## What Changes

- New `aiditor-services/src/agents.rs`: an `Agents` service, shaped like `Terminals`, that runs `AgentOp`s.
  - `AgentOp::Spawn` starts the agent's command in the primary repo with the given environment layers on top of the editor's, sends `initialize` advertising no fs or terminal capabilities, then `session/new` with `cwd`, and answers with `Msg::AgentStarted` carrying the `session/new` response (session id, modes) or why it failed.
  - Each stderr line comes back as `Msg::AgentStderr`, the exit as `Msg::AgentExited`, after its stderr.
  - Agent-to-client requests come back as `Msg::AgentRequest` and are kept by request id until an `AgentOp::Reply` answers them.
  - `AgentOp::Kill` and `Agents::kill_all` end the agent's process group: SIGTERM, SIGKILL after 100 ms.
- The core depends on `agent-client-protocol-schema`; `Msg` and `Effect` carry its types as they are. The agent's request travels in a `Request` newtype that compares by its `Debug` text, since the schema's `AgentRequest` has no `PartialEq` and `Msg` derives it.
- The core rejects every agent request with "method not found" until the modes epic handles permissions; other agent messages are ignored until the session task.
- The app runs `Effect::Agent` and kills every agent on quit.
- A minimal test agent binary built on the ACP crate (`aiditor-test-agent`), which the fake agent epic grows into the scripted fake.

## Capabilities

### New Capabilities
- `agent-processes`: spawning ACP agent processes, the handshake and session open, stderr, exit, agent requests and replies, killing.

### Modified Capabilities

## Impact

- Dependencies: `agent-client-protocol` 3.5.0 and `tokio-util` (compat) in services; `agent-client-protocol-schema` 1.12.0 (no default features) in the core.
- `aiditor-core/src/lib.rs`: `AgentId`, `AgentOp`, `Effect::Agent`, four `Msg` variants.
- `aiditor/src/editor.rs`: runs agent effects, kills agents on quit.
- No user-visible command yet: "Start an agent session from the editor" (#280) wires it up.
