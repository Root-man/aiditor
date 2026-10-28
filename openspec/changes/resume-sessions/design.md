# Design

## Context

`agent-start` builds a `Session` and a held `AgentOp::Spawn`; the agent service's `connect` sends `initialize` and `session/new` and answers `Msg::AgentStarted` with the `NewSessionResponse`. Notifications are forwarded as `Msg::AgentUpdate` whenever they arrive, `session` or not, and `agent_update` renders them as long as the session hasn't ended. Probing the real adapters on this repo: both answer `session/load` with `configOptions` and `modes` (no `sessionId` from codex-acp), and replay history first as `user_message_chunk` (one whole message per chunk, each with its own `messageId`), `agent_message_chunk`, `agent_thought_chunk`, `tool_call`, `tool_call_update`, and from codex `session_info_update` and `available_commands_update`.

## Goals / Non-Goals

**Goals:** load through the same spawn, session and rendering paths a new session uses.

**Non-Goals:** reloading an `[ended]` session in place (`RET` still shows its dead buffer; kill it and `g` to load it again); `additionalDirectories` (the preamble epic adds them to new and loaded sessions alike); a committed load transcript.

## Decisions

- **`Spawn` gains `session: Option<SessionId>`** rather than a new `AgentOp::Load`. The spawn, env capture release, process handling and connection are identical; only the request after `initialize` differs. `connect` branches there and builds a `NewSessionResponse` from the requested id and the `LoadSessionResponse`'s `config_options` and `modes`, so `Msg::AgentStarted`, `Session::session` and everything reading them stay as they are.
- **Ordering:** the connection dispatches incoming messages in order, so the replayed notifications reach the core's channel before `block_task` on `session/load` wakes and sends `AgentStarted`. The core doesn't depend on it anyway: updates render while the session is `[starting]`.
- **`Session::load: Option<SessionId>`** marks a loading session. `open_session` matches it as well as the answered id, so the row shows `[starting]` (the state logic already says `[starting]` while `session` is `None`) and a second `RET` switches to the buffer. `agent_started` skips `default_mode` when it is set.
- **Shared start:** `agent_start`'s body becomes a `Core`/`Ctx` helper taking the agent name, the session to load and the buffer title; `agent-start` passes none, `sessions_visit` passes the row's session id and its listed title, found in the row agent's answer.
- **User chunks** go through `output` with no stream kind as `> <text>`, then the text after `> ` is rendered with `render(…, thought: false)`, the same call `agent_send_input` makes on a sent prompt. `output` already ends a stream and adds the blank line. shortcut: one block per chunk; join chunks by `messageId` if an adapter ever splits a message.

## Risks / Trade-offs

- [Long histories render update by update on load] → the same path as a live turn, which already streams many tool calls; measure if a big session stalls.
- [A session listed under the primary root but recorded with another `cwd`] → loaded with the primary root as `cwd`, as the spec decides; the adapters filtered the list by that `cwd`.
