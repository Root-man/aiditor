## Why

The session buffer keeps `plan` and `current_mode_update` updates unrendered, so the agent's to-do list and the approval mode it runs in are invisible (#295, epic M3 session buffer). Both change during a turn and matter at a glance: the plan shows progress, the mode shows how freely the agent may act.

## What Changes

- The first `plan` update adds a plan block above the prompt line: a `Plan` line, then one line per entry, `<content> (<status>, <priority>)` with the ACP names, a rule beside every line. Each later `plan` replaces the entries in that same block, wherever it sits in the transcript.
- The session buffer's mode line shows the agent's current mode as ` [<mode id>]` after the buffer name: from `session/new`'s current mode once the session is ready, then following every `current_mode_update`.
- Not here: cycling modes (the modes epic), plan entries as buttons.

## Capabilities

### New Capabilities

### Modified Capabilities
- `session-buffers`: plans render as a block updated in place; the mode line shows the current mode; only remaining updates stay unrendered.

## Impact

`aiditor-core`: `agents.rs` (plan block, mode tracking), `buffer.rs` (a field for the mode), `display.rs` (mode line); `docs/editor.md`. No frontend change: rules and faces already draw.
