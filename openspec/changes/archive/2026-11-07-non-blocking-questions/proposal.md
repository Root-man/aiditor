# Proposal

## Why

codex-acp's `request_user_input_async` asks questions without waiting: its `elicitation/create` carries `_meta.codex.delivery: "async"`, the turn goes on, an accepted answer is steered into the turn, and the adapter withdraws open questions with `$/cancel_request` when the turn ends. Today the editor treats such a question as blocking: the mode line says `● awaiting answer` while the agent is still working, point jumps into the block and the turn's end cancels it. Issue #412 draws it as frame 6f.

## What Changes

- A question whose request carries `_meta.codex.delivery: "async"` is non-blocking: the session's status stays what it would be without it (`● working` during a turn), and the mode line adds `<n> question` (`<n> questions`) in `status-waiting` for the open non-blocking questions.
- A non-blocking question never moves point; the echo line says `Question waiting: C-c C-q goes to it` when the session is in the selected window. Output keeps streaming below the block, which stays where it arrived.
- A single non-blocking question's hint is frame 6f's: `C-c C-q go to it   C-c C-k skip   an answer joins this turn`.
- At the turn's end a non-blocking question still open closes as `○ withdrawn`, as codex-acp's own withdrawal closes it, whichever comes first.
- The sessions list shows `[working]`, not `[waiting]`, for a session with only non-blocking questions.

## Capabilities

### New Capabilities

### Modified Capabilities
- `agent-questions`: non-blocking questions.
- `session-buffers`: the mode line's question count.

## Impact

- `crates/aiditor-core/src/agents/questions.rs`, `agents.rs`.
- `docs/editor.md`.
- No new dependencies.
