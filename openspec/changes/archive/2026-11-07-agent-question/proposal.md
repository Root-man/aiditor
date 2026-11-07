# Proposal

## Why

claude-agent-acp drops `AskUserQuestion` from the model's tools unless the client advertises `elicitation.form`, and codex-acp asks its questions through the same request, so today neither agent can ask the user anything. Issue #407 starts epic #406 with the single-choice question, drawn as frames 6d and 6f show it.

## What Changes

- The agent service advertises `clientCapabilities.elicitation.form` (not `url`), passes `elicitation/create` to the core like a permission request, and tells the core when the agent withdraws a pending request with `$/cancel_request`, answering it with the request-cancelled error itself.
- A single-choice question (one choice field, at most one companion own-answer field) shows as a block above the prompt line, in the permission block's faces: `Question: <message>`, the field's title when it adds one, a row per option with its description, an `o. Own answer` row when there is a companion, and a key-hint row. While it names a `toolCallId` its block stands in for that call's row in a tool group.
- A digit or `RET` on an option answers at once with that option's value; `o` reads an own answer in the minibuffer and sends it in the adapter's shape (`question_<n>_custom` for claude-agent-acp, `None of the above` plus the note field for codex-acp). The block becomes one dim line.
- `C-c C-k` skips the oldest pending request: a question is answered `decline`, a permission request rejected as before. `C-c C-y` answers the oldest permission request, passing questions by.
- `C-c C-c`, the turn's end and killing the session buffer answer pending questions `cancel`; killing the buffer now also answers pending permission requests `cancelled` before the agent is stopped. A withdrawn request's block closes.
- Point moves to a new question only when the prompt is empty; otherwise the echo line says `Question waiting: C-c C-q goes to it`, and `C-c C-q` goes to the oldest question.
- The mode line shows `● awaiting answer` while a question is pending.
- A form or URL request this change can't draw is answered `decline` with an echo line.
- The fake agent gets a `question` step, with an optional withdrawal after a delay.
- `docs/editor.md` records the question rule and decisions; AGENTS.md already lists frames 6d to 6f.

## Capabilities

### New Capabilities
- `agent-questions`: questions from agents (`elicitation/create`): which requests are questions, how a question shows, how it is answered, skipped, cancelled and withdrawn.

### Modified Capabilities
- `agent-processes`: `initialize` advertises `elicitation.form`; elicitations reach the core; withdrawn requests reach the core and are answered by the service.
- `session-buffers`: `C-c C-k` also skips questions and `C-c C-y` passes them by; killing a session buffer cancels its pending requests; the status shows `● awaiting answer`.
- `fake-agent`: a `question` step.

## Impact

- `crates/aiditor-core/src/agents.rs`, new `agents/questions.rs`, `agents/groups.rs`, `lib.rs` (`Msg`), `builtins.rs`, `keymap.toml`.
- `crates/aiditor-services/src/agents.rs`, `bin/aiditor-fake-agent.rs`, `tests/agents.rs`, `tests/scenario.rs`.
- `docs/editor.md`.
- No new dependencies.
