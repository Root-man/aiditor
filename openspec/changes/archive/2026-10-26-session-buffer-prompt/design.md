# Design

## Context

`agent-start` (#280) keeps `Core::agents: BTreeMap<AgentId, Session>`, each with a read-only stderr log buffer that is the session's handle: `sync_agents` drops a session whose buffer is gone and sends `AgentOp::Kill`, and `live_agent` makes `kill-buffer` ask. Updates are kept in `Session::updates`. The agent service drives each connection inside `connect_with` and has no way to send anything after `session/new`. Buffers are read-only only as a whole (`Buffer::read_only`); markers follow edits with `Stay` or `Advance` insertion types; `append` writes past read-only without undo.

## Goals / Non-Goals

**Goals:** a usable comint-style session buffer, and a core and service API for sending prompts that later tasks (#286, #288) drive from key scripts.

**Non-Goals:** markdown rendering, faces, tool calls, plans, modes, permissions (requests stay refused), the roots preamble, rendering only once per frame, input history (`M-p`).

## Decisions

- **One boundary marker per buffer.** `Buffer` gains `prompt: Option<MarkerId>`, a `Stay` marker at the prompt area's start; `raw_insert` before it and `raw_delete` starting before it fail with the new `Error::TextReadOnly` (`Text is read-only`, as Emacs says). `Stay` keeps typing at the area's start inside the area. Alternative: a `read-only` text property, as the display model sketches; it needs property lookups on every edit for one boundary that only moves forward.
- **Output goes above the prompt line.** `Buffer::insert_output(at, text)` inserts past the boundary without undo and shifts every undo and redo position at or after `at`, so the prompt area's undo survives streamed output. The prompt line `> ` sits just before the marker, so output's position is the marker minus 2.
- **Layout.** The transcript is empty or ends with a blank line (`\n\n`); a new block (an echo, a reply, a turn-end line) is inserted at the transcript's end as `text\n\n`, and a chunk continuing the streamed reply is inserted before that trailing `\n\n`. A send leaves the input where it is as the echo: it appends `\n\n> ` at the end, moves the marker to the end, moves point there and clears the buffer's undo (the old prompt area's history would replay into read-only text). So the echo reads `> hello` like a frozen prompt, as in comint.
- **Turn state** lives in `Session`: `buffer` is now the session buffer, `log` the stderr buffer, plus `turn: bool` and `streaming: bool`. `RET` checks ended, ready and busy in that order before sending, so a failed start says it ended.
- **Ops.** `AgentOp::Prompt { id, request: PromptRequest }` and `AgentOp::Cancel { id, session: SessionId }`, answered by `Msg::AgentPrompted { id, result: Result<StopReason, String> }`. The core builds the ACP request, so the service needs no session state of its own.
- **Service keeps the connection.** `connect` stores the `ConnectionTo<Agent>` in the handle once `session/new` is answered, before sending `AgentStarted`, so a prompt the core sends after ready always finds it. Prompts use `on_receiving_result`, whose callback runs before the dispatch loop takes the next message; with the notification handler awaiting its send, `AgentPrompted` reaches the core after every update the agent sent before its answer. A connection that closes before answering sends no answer; the exit ends the session.
- **Handle moves to the session buffer.** `sync_agents` and `live_agent` look at `Session::buffer`, now the session buffer; stderr goes to `log` while it exists.

## Risks / Trade-offs

- [Every chunk edits the rope at once rather than once per frame] → shortcut: chunks are small and few per frame; batch when long replies show up in profiles.
- [Output inserted while a region spans the boundary] → only prompt-area edits are allowed, so the region commands' deletions starting in the transcript fail cleanly with `Text is read-only`.
- [A turn whose agent dies never gets an answer] → the exit ends the session, which makes `RET` say it ended.
