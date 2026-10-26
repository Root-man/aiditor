# Proposal

## Why

`agent-start` (#280) opens an ACP session, but nothing in the editor can talk to it: there is no buffer to read the agent's replies in and no way to send a prompt. This change adds the session buffer with a comint-style prompt (#292), which #286 and #288 need to drive a turn from the editor.

## What Changes

- `agent-start` creates and shows a session buffer `*<agent>*<<workspace>>`, which becomes the session's handle in place of the stderr log buffer: killing it (asked first while the agent is live) or its workspace stops the agent. Killing the log buffer no longer stops the agent and no longer asks.
- The session buffer ends in a prompt line `> `; everything before the prompt area is read-only (`Text is read-only`), and output is inserted above the prompt line.
- New mode `agent-session-mode`: `RET` (`agent-send-input`) sends the prompt area's text as a `session/prompt` and leaves it above the marker as the echo; `C-j` and `S-RET` insert a newline; `C-c C-c` (`agent-cancel`) sends `session/cancel`. `RET` while a turn runs echoes `<agent> is busy`.
- Agent message chunks are appended as plain text as they stream. A turn that ends other than `end_turn` gets a `Turn ended: <reason>` line; an error reply gets `Error: <message>`. Other updates stay kept and unrendered.
- The agent service gains `AgentOp::Prompt` (answered with `Msg::AgentPrompted`, after the turn's updates) and `AgentOp::Cancel`.

## Capabilities

### New Capabilities
- `session-buffers`: the session buffer, its read-only transcript and prompt area, sending and cancelling turns, rendering message chunks and turn ends.

### Modified Capabilities
- `agent-sessions`: the session buffer, not the log buffer, is the session's handle.
- `agent-processes`: prompts and cancels reach the agent; prompt answers stream to the core after the turn's updates.

## Impact

- `aiditor-core`: `buffer.rs` (a prompt boundary that makes text before it read-only, inserting output above it), `agents.rs` (session buffer, send, cancel, rendering), `lib.rs` (`AgentOp::Prompt`/`Cancel`, `Msg::AgentPrompted`), `builtins.rs`, `keymap.toml`.
- `aiditor-services/src/agents.rs`: keeps each agent's connection to send prompts and cancels.
- `docs/editor.md`: keys and behaviour under ACP integration.
- No frontend change: the session buffer is a text buffer and `S-RET` already reaches the core.
