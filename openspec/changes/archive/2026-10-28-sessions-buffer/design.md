# Design

## Context

`AgentOp::List { request, .. }` (#322) answers `Msg::AgentListed { request, result }`; the core ignores it. Agent spawns wait for the project's environment capture by being held in their `Session` and sent by `release_agents` when `captured(project)` answers. `workspace_list.rs` is the house pattern for a read-only list buffer with `n`/`p`/`RET`/`q`, keeping row lines in a `Buffer` field.

## Goals / Non-Goals

**Goals:** the `*Sessions*` buffer and its keys, matching the house list-buffer pattern, testable through the harness.

**Non-Goals:** loading sessions (#103); live-state marks beyond `[open]`; columns that adapt to the window width.

## Decisions

- **State in `Core`, not `Buffer`.** A `session_lists: HashMap<BufferId, SessionList>` in `Core` holds per buffer the project, per registry agent its name, latest request number, held `AgentOp` and answer (`Option<Result<Vec<SessionInfo>, String>>`), and the row lines with `(agent, SessionId)`. The buffer itself stays a plain read-only buffer; `workspace_rows` is specific to its list and a second such field would grow `Buffer` for one mode. Entries for killed buffers are dropped when the next answer, request or capture arrives, so a list killed while waiting for the capture is never sent.
- **One request number per agent.** `AgentOp::List` carries one number and no agent, so each agent's request gets its own number from a `Core` counter; an answer is matched by scanning the lists for that number, and a number no list holds is dropped. `g` hands out new numbers, which is what drops late answers.
- **Held lists reuse `release_agents`.** Each `SessionList` holds its unsent ops; `release_agents(project)` also sends the held list ops of that project with the captured variables first, the same as spawns. Environment: captured, then the registry's `env`, then the project file's — the same order as `agent_start`.
- **`jiff` for time.** `Timestamp::from_str` parses RFC 3339 with any offset and fraction; `to_zoned(zone)` and `strftime("%Y-%m-%d %H:%M")` format it. Pure `timestamp(updated)` (for sorting) and `local(time, zone)` (for showing) take the zone, so tests pass a fixed one; the core passes `TimeZone::system()`. `jiff` 0.2 is already in the lockfile through another crate, so it adds no new download. Alternative `chrono` is not in the tree; `time` needs features for RFC 3339 and local offsets that are unsound on macOS threads.
- **Render whole text.** Each answer re-renders the full text with `set_text`; lists are tens of rows. Point is restored by row key, through the window showing the buffer when it is shown, else the buffer's point marker.
- **`[open]` from the core's sessions.** A session is open when `core.agents` has a session with the same agent name, `session.session_id` equal to the listed id, and its buffer exists.

## Risks / Trade-offs

- [Agent fixed-width padding uses char counts] → names are ASCII registry keys in practice.
- [An agent registered twice under different names lists the same sessions twice] → accepted; each row names its agent.
