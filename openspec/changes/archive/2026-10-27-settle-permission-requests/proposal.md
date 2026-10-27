## Why

ACP requires the client to answer pending permission requests with `cancelled` after `session/cancel`, and an agent must never wait on a request the client will not answer. #307 settles requests on `C-c C-c` and exit, but a permission request that arrives after the session ended, or after its session buffer was killed while the agent is being stopped, is still refused with "method not found" instead of being cancelled (#308).

## What Changes

- A `session/request_permission` for a session that has ended, or for an agent whose session was forgotten because its buffer was killed, is answered with the `cancelled` outcome and not shown.
- Requests before `session/new` is answered, and every other agent request, are still refused with "method not found".
- `C-c C-c` and agent exit keep settling pending requests as built in #307; tests cover both acceptance criteria.

## Capabilities

### New Capabilities

### Modified Capabilities
- `agent-processes`: the core cancels permission requests from ended or killed sessions instead of refusing them.
- `session-buffers`: cancelling ends pending permission requests, and late ones are cancelled without a block.

## Impact

`crates/aiditor-core/src/agents.rs` (`agent_request`), the refusal tests in `crates/aiditor-core/src/lib.rs` and `crates/aiditor-services/tests/agents.rs`, and `docs/editor.md`.
