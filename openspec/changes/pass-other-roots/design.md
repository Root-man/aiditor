# Design

## Context

The core builds `AgentOp::Spawn` in `start_session` (crates/aiditor-core/src/agents.rs); the service (crates/aiditor-services/src/agents.rs `connect`) initializes the agent and sends `session/new` or `session/load`, answering `Msg::AgentStarted`. Only the service sees the agent's capabilities.

## Goals / Non-Goals

**Goals:** pass other roots, note a missing capability, send the preamble once per new session.

**Non-Goals:** hiding a replayed preamble on load; updating a running session's roots; validating root paths (the project file already holds absolute paths).

## Decisions

- **`AgentOp::Spawn` gains `roots: Vec<PathBuf>`**, filled from `project.roots` at start. The service drops them unless `initialize` advertised the capability.
- **A new `Msg::AgentUnscoped { id }`** sent just before `AgentStarted` when roots were dropped. Alternative: a field on `AgentStarted`, which touches ~30 constructions in tests for a rarely true flag.
- **Preamble stored on `Session`** as `Option<String>`, built at start for new sessions only and taken by the first prompt, so later prompts and loads never carry it. Sent as its own `ContentBlock::Text` before the user's; the buffer echoes only the input as today.
- **Format:** `Project roots:` then one `- <path>` line per root, ` (primary)` after the primary's path, `: <role>` when the role is non-empty.
- **Fake agent:** script key `directories: bool`; reports `dirs=` on new and load.

## Risks / Trade-offs

- [A replayed preamble shows on load] → accepted, noted in docs/editor.md.
- [A first prompt the agent rejects consumes the preamble] → the preamble is taken when sent; a refused prompt is rare and the roots still reach the agent through `additionalDirectories`.
