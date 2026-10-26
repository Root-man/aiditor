# Design

## Context

Agent requests reach the core as `Msg::AgentRequest` and are refused in `lib.rs`'s dispatch. The session buffer already shows blocks above the prompt line that are rewritten in place (tool calls, the plan) through a start marker and a length, and `AgentOp::Reply` carries any `ClientResponse`.

## Goals / Non-Goals

- Goals: show, answer and cancel permission requests in the session buffer, keyboard only.
- Non-Goals: `allow_always` bookkeeping on the client (the agent remembers it), number keys, buttons or mouse, merging the request's tool call into the tool call block, answering requests from other buffers.

## Decisions

- **Pending requests per session, oldest first.** The session keeps a `Vec` of pending requests: request id, title, options, block start marker and length. `C-c C-y`/`C-c C-n` take the first; `RET` finds the block point is in. A queue keeps "oldest first" trivial; the agents ask one at a time per turn in practice, so a linear search is fine.
- **The block is rendered like the plan.** Both are rows of `<text> (<dim suffix>)` with a rule beside each line, so the plan's renderer takes rows (text, length before the suffix) and serves both.
- **Answered block via `rewrite`.** The block is rewritten to its one answer line in place, as tool calls and the plan are, then its marker dropped. The answer line stays plain.
- **`RET` dispatch in `agent-send-input`.** Rather than a text-property keymap the buffer model lacks, `agent-send-input` first checks whether point is on an option line of a pending block, then sends the prompt as before. Alternative, a new property type carrying keys, is more machinery for one key.
- **Cancel answers pending requests.** ACP requires the client to answer pending permission requests `cancelled` after `session/cancel`; `agent-cancel` does so after sending the cancel. On exit the requests are gone with the process, so only the blocks are rewritten.
- **Refuse what can't be shown.** A permission request for an agent whose session isn't ready or has ended, and every other request, is refused as before.

## Risks / Trade-offs

- [A request arrives for a session whose buffer is not shown] → It still renders there; the keys work once the user switches to it. Notifications come after the MVP.
- [The title duplicates the tool call header just above] → Acceptable: the prompt must name its tool when several are pending.
