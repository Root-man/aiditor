# Design

## Context

Loading a listed session (#103) already does most of this: `start_session` creates a hidden buffer, the spawn sends `session/load`, the replay streams in as updates, and `agent_started` shows the buffer if the window still shows where the load started. The core keeps each message's `messageId` (#363). claude-agent-acp 0.97's `session/fork` copies the transcript (cut at the message's uuid when `_meta.jetbrains.air.fork` names one) and answers the new id without replaying anything; a `session/load` of that id in the same process replays it.

## Goals / Non-Goals

**Goals:** fork through the load path with the least new state.

**Non-Goals:** a key binding (M-x only, like `agent-start`); forking from `*Sessions*`; message fingerprints or occurrences in the fork meta; copying transcript text client-side.

## Decisions

- **Fork, then load.** The service sends `session/fork` and then `session/load` of the answer, so the agent itself replays what it kept: tools, groups, message ids and timings work as in any loaded session, and Codex's cut at the turn's end shows as it really is. Alternative, copying the source buffer up to the message, loses the structured state of everything copied and can disagree with the agent's cut. Cost: one replay, and the agent must also advertise `loadSession` (both adapters do); one that doesn't fails with `agent can't load sessions`.
- **`AgentOp::Spawn { session: Open }`** with `Open::New`, `Open::Load(id)` and `Open::Fork(id, Option<MessageId>)` replaces `Option<SessionId>`, so each way of opening is one match arm in `connect`.
- **`Session::load` becomes `Option<(Option<SessionId>, BufferId)>`.** A fork's id is unknown until it answers, so it is `None` and `open_session` doesn't match the source's row in `*Sessions*` while the fork loads. The `from` buffer is the source buffer, so the fork shows if the user stayed there. A loaded or forked session keeps its own mode, as now.
- **The fork point** is the message `Session::message_lines` uses for the bar (last message starting at or before point); point at or after the prompt start forks the whole session.
- **Failure kills the hidden buffers.** `agent_started`'s error arm, for a fork, kills the session buffer and log instead of keeping them `[ended]`; `sync_agents` then forgets the session and kills its process, as for any killed session buffer. A fork is told from a load by its `load` holding no id, so no new field is needed.
- **Roots** come from the project as it is now, as any start does (#329), not frozen from the source.
- The fake agent's fork answers `fake-fork` and cuts the next load's history after the last update with the fork point's id.

## Risks / Trade-offs

- [codex-acp's load of a just-forked thread isn't checked here, and it isn't installed on this machine] → the issue's last criterion is a manual check on macOS against both adapters.
- [Prompts aren't fork points: claude-agent-acp would cut right after one, leaving an unanswered prompt, while codex-acp keeps its whole turn, and live-sent prompts have no id at all] → messages remember whether they are the user's, and a prompt echoes `Fork at an agent message`; forking at the reply before works.
- [A failed fork's stderr log is killed with it] → the error is echoed and kept in `*Messages*`.
