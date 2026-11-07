# Design

## Context

Permission requests already travel the whole path: the service keeps each incoming request's `Responder` in a pending map and sends `Msg::AgentRequest`; the core shows an `Ask` block, keeps it in `Session::asks` (oldest first, a `Row::Ask` in the tool group), and answers with `AgentOp::Reply`. Turn end, `agent-cancel` and exit already walk `asks`. A question needs the same lifecycle with a different body and answer.

## Goals / Non-Goals

**Goals:** single-choice questions from both adapters, answered, skipped, cancelled and withdrawn, sharing the permission block's code.

**Non-Goals:** multi-select and previews (#408), several questions (#409), forms (#410), URL mode (#411), non-blocking behaviour beyond what falls out (#412), the answered line's final look (#413).

## Decisions

1. **One `Ask` list for both kinds.** `Ask` gains a body: the permission options, or a parsed `Question` (choice key, options, companion). Oldest-first order, group rows, waits, turn end and exit then cover questions with no new code. Alternative, a separate `questions` list: duplicates every walk.

2. **Parsing.** A form request is a single-choice question when its properties are one string field with `oneOf` (or `enum`) and at most one companion: a string field without choices whose key is `<choice>_custom` (claude-agent-acp) or whose `_meta.codex.role` is `user_note` (codex-acp). Own answer: claude `{<key>_custom: text}`; codex `{<key>: "None of the above", <note>: text}`, and codex's own `None of the above` option is hidden since `o` stands in for it. Without a companion there is no `o` row. Anything else (several choices, arrays, other types, URL mode) is declined with the echo line `Can't show <agent>'s question yet; declined`.

3. **Block text, frame 6d.** Each line carries `Indent(1)` and `Background(permission)` line decorations, the box; a leading space is its padding. ` Question  <message>` with `Question` in `status-waiting` bold, the choice field's `title` when present and different from the message (codex puts the question there and a generic message), `   1   <title>  <description>` with the number in a `key-hint` chip and titles padded so descriptions line up, the same with ` o ` for the own answer, and the dim hint with its keys in `status-waiting` bold (`1 answer` for one option). The frame builder adds `Background(region)` to the option line at point. Point lands on the column before the chip, where frame 6d's cursor is. Answered line, dim: `✓ <message>  <answer>`, `– <message>  Skipped`, `✗ <message>  Cancelled`, and for any withdrawn request `○ withdrawn`.

4. **Keys.** `1`..`9` and `o` in `agent-session-mode` run `agent-question-key`: on a question block's lines a digit answers that option and `o` runs `agent-answer-own` (prompt `Own answer: `, empty refused); elsewhere they self-insert. `C-c C-q` is `agent-goto-question`. `agent-reject-once` answers the oldest ask of either kind; `agent-allow-once` the oldest permission ask. After an answer from inside the block, point goes to the end of the prompt.

5. **Point on arrival.** If the prompt area is empty, point moves to the first option row in each current-workspace window showing the session and in the buffer's own point; else, if the selected window shows the session, the echo line says `Question waiting: C-c C-q goes to it`.

6. **Withdrawal in the service.** On each incoming request the service takes `responder.cancellation()` and spawns a task awaiting `cancelled()`; if it was cancelled and the responder is still pending, the service removes it, responds with `Error::request_cancelled()` and sends `Msg::AgentWithdrawn { id, request }`. The core closes that ask's block without replying. This covers permission requests too.

7. **Killing the buffer.** `sync_agents` replies `cancel`/`cancelled` for each pending ask before `AgentOp::Kill`, so the agent hears the answer before its process gets SIGTERM.

## Risks / Trade-offs

- [The cancellation watcher task waits until the request settles or the connection drops] → it checks `is_cancelled()` and the pending map, so a settled request does nothing; tasks end with the connection.
- [Codex's real shape was read from its source (agentclientprotocol/codex-acp main), not recorded] → the acceptance criterion asks for a check against the real adapters on macOS.
- [Digits 1–9 only] → options past 9 are answered with `RET`.
