# Design

## Context

`Ctx::notify(buffer, title, body)` (#524) checks the setting and pushes `Effect::Notify` tagged with the buffer. The core keeps `window_active` from the frontend and `shown_buffers()` lists the current workspace's shown buffers. Each event already has one handler: `agent_request` (permissions, questions), `agent_prompted` (turn end), `agent_started`'s error arm (failed start) and `agent_exited` (exit).

## Goals / Non-Goals

**Goals:** one notification per event, from the handler that already handles it, suppressed while the session is seen.

**Non-Goals:** withdrawing notifications, non-blocking questions, terminals.

## Decisions

- **Post from the handlers, not by diffing `activity()`.** The handlers know the body (tool call title, question, stop reason) and whether a turn was cancelled; a diff of `Activity` would lose both.
- **`Ctx::notify_unseen(buffer, title, body)`** in `notifications.rs`: posts unless the window is active and `shown_buffers()` holds the buffer. #526 reuses it. `notify-test` keeps posting regardless.
- **`Ctx::notify_session(id, body)`** in `agents.rs` names the title `<agent> · <workspace>` from the session's start-time names, as its buffer does.
- **Permission body.** `show_permission` returns the title it computed, so the notification matches the block.
- **Turn end.** `Ok(EndTurn)` posts `Turn ended`; `Ok(Cancelled)` posts nothing; any other result posts the line the session buffer gets.
- **Exit.** Posted with the `*Messages*` line, inside the existing "first time ended" check, so the kill that follows a failed start doesn't post twice; a session killed with its buffer is gone before the exit arrives.

## Risks / Trade-offs

- [A session in another workspace that is visible on another screen still posts] → aiditor has one window; accepted.
