# Proposal

## Why

A session transcript is one long run of text: nothing tells where a message starts, and getting back to the last reply means scrolling past tool calls. ACP gives every chunk a `messageId`; aiditor drops it. Later session work (timings, the collapsed tool groups) also needs messages as units.

## What Changes

- The core keeps a session's messages in order, with each one's ACP `messageId`, for live and replayed chunks. A prompt sent live has no id and takes its place after the last agent message.
- A chunk with a different `messageId` starts a new block, where consecutive chunks used to join whatever their id.
- The message at point, the last one starting at or before point, gets a bar in the left margin in `message-at-point`, per window, as frame 6 draws it.
- `C-c C-p` (`agent-previous-message`) and `C-c C-n` (`agent-next-message`) move between messages, past tool calls and thoughts.
- **BREAKING** `agent-reject-once` moves from `C-c C-n` to `C-c C-k`.

## Capabilities

### New Capabilities

### Modified Capabilities

- `session-buffers`: a new `messageId` starts a block; messages are kept, marked and moved between; `agent-reject-once` moves to `C-c C-k`.
- `agent-processes`: the permission scenario names the new key.
- `frontend-rendering`: a bar decoration is drawn at the line's left edge.

## Impact

`crates/aiditor-core`: `agents.rs` (messages, `Stream`, commands), `display.rs` and `frame.rs` (the bar), `buffer.rs` (`Decoration::Bar`), `builtins.rs`, `keymap.toml`. `crates/aiditor`: `element.rs` paints the bar. `docs/editor.md`: the protocol table and the ACP section.
