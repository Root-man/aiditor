# Proposal

## Why

The core refuses every `session/request_permission` with "method not found", so any tool call the agent's mode doesn't auto-approve fails. Issue #307 shows the request in the session buffer and lets the user answer it from the keyboard.

## What Changes

- A permission request from a ready session adds a block above the prompt line: `Permission: <title>`, then a numbered line per option, `<n>. <name> (<kind>)`, the kind in `dim`, a `dim` rule beside every line.
- `C-c C-y` (`agent-allow-once`) answers the oldest pending request with its first `allow_once` option, `C-c C-n` (`agent-reject-once`) with its first `reject_once` option, from anywhere in the session buffer. `RET` on an option line of any pending request picks that option; elsewhere `RET` still sends the prompt.
- Answered, the block becomes one line, `Chose <name> (<kind>) for <title>`.
- Several requests can be pending; the keys answer the oldest first.
- `C-c C-c` also answers every pending request `cancelled`, as ACP requires on `session/cancel`; an exit leaves them unanswered. Both turn the block into `Cancelled permission for <title>`.
- Other agent requests, and permission requests for a session that isn't ready or has ended, are still refused with "method not found".
- The transcript replay answers each recorded permission request with `C-c C-y` and asserts every one got an answer line.
- docs/editor.md records the keys and behaviour.

## Capabilities

### New Capabilities

### Modified Capabilities
- `session-buffers`: adds permission prompts and their keys; `C-c C-c` cancels pending requests.
- `agent-processes`: the core no longer refuses every agent request.
- `acp-transcripts`: the replay answers permission requests through the session buffer.

## Impact

- `crates/aiditor-core`: `agents.rs` (pending requests per session, the block, the commands, cancel and exit), `lib.rs` (dispatch), `builtins.rs`, `keymap.toml`.
- `crates/aiditor-services/tests`: `scenario.rs` (fake agent end to end), `transcripts.rs` (replay answers).
- No service or protocol changes: `AgentOp::Reply` already carries a `ClientResponse`.
- docs/editor.md.
