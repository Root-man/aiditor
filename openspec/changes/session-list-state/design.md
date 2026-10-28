# Design

## Context

`render_sessions` labels a row `[open]` when `Core::open_session` finds a session buffer for the row's agent and session id. A session's state lives in `agents::Session`: `ended`, `session` (the `session/new` answer), `asks` (pending permission requests) and `turn`. Those fields change in many handlers (start, prompt, request, answer, cancel, exit, kill).

## Goals / Non-Goals

**Goals:** rows show the state; the list follows it without `g`.

**Non-Goals:** state for sessions running outside the editor; a state column or faces.

## Decisions

- **One state function next to `open_session`.** `Core::session_state(agent, id) -> Option<&'static str>` returns the label of an open session, in the spec's precedence order. `open_session` stays for `RET`.
- **Diff after each update instead of notifying from each handler.** `SessionList` keeps the labels it last rendered per row. After `sync_agents` in `Core::update`, each live list recomputes its rows' labels and re-renders only when one differs. Hooking every handler that touches `turn`/`asks`/`ended` would scatter calls and miss future ones; the check costs one lookup per row per message, which is small next to the rest of `update`. Alternative of re-rendering every list every message was rejected: it rewrites buffer text on every keystroke.
- **`[starting]` is matched but not yet reachable.** Rows match open sessions by session id, which a starting session gets only from the `session/new` answer, so until `session/load` exists a starting session's row isn't listed. The branch costs one line and is right once loading lands.

## Risks / Trade-offs

- [Per-message check over all rows] → bounded by the listed sessions; lists are short.
