# Design

## Context

Fork (#349) spawns a hidden session buffer, sends `session/fork` (cut at a message through `_meta.jetbrains.air.fork`) then `session/load`, and on the load's answer shows the buffer if the source is still selected; a failed fork kills its buffers. Rollback reuses all of that and changes only what happens at the answer. Both adapters advertise `sessionCapabilities.delete`: claude-agent-acp 0.97's `session/delete` tears down a loaded session and removes its stored transcript; codex-acp 2.2.2 advertises it (checked by `initialize` on macOS), its effect on disk not checked.

## Goals / Non-Goals

**Goals:** rollback through the fork path with one new `Open` variant and one session flag.

**Non-Goals:** a key binding (M-x only, like fork); reverting files; rolling back from `*Sessions*`; reporting a failed delete beyond the log.

## Decisions

- **Fork first, swap on success.** The source stays usable until the fork has loaded, so a failure loses nothing. Alternative, clearing the source buffer and loading into it, loses the transcript on failure.
- **`Open::Rollback(SessionId, MessageId)`.** The service handles it as `Open::Fork(source, Some(at))`; the core reads it to set `Session::rollback`, so `start_session`'s signature stays. The fork's `load` holds the source buffer as `from` already, since the command runs in the selected window's session buffer, so no buffer id is stored.
- **The swap**, in `agent_started`'s success arm: each window of `core.windows` and of every inactive workspace's windows that shows the source shows the fork; then the source is marked for deletion and its buffer and log are killed (`sync_agents` cancels its asks and sends `AgentOp::Delete` instead of `Kill`) and the fork's buffer and log are renamed to the freed names. If the source buffer is already gone nothing is swapped: the fork keeps its own name, unshown.
- **Delete through the source's own process, then stop it** (`AgentOp::Delete`). Found in review: deleting through the fork's process left the source process alive with the session loaded, and claude-agent-acp's delete only tears a session down in the process that has it loaded, so the transcript could be written again and the source still listed. The service keeps whether the agent advertised delete per handle; without it, or without an open connection, Delete is a Kill. It waits at most 5 s for the answer, then stops the agent anyway; the handle stays until quitting, so a quit meanwhile still kills it. A failed delete is a `tracing::warn`.
- **Open sessions buffers drop the source's row** at the swap, so an old list can't load a deleted session (which failed with `Resource not found` and left an `[ended]` row). shortcut: assumes the agent can delete; `g` lists it again if it can't.
- **The message at point** is found by one helper shared with `agent-fork-session`; each command words its own refusals. The prompt line counts as no message for rollback (fork takes it as "the whole session").
- **The question** is asked through `read_string` with `no`/`yes`, as `kill-buffer` does; the answer re-runs the command with point unchanged.

## Risks / Trade-offs

- [Local-command replies (claude's `/rename` output) arrive as agent messages whose id isn't an assistant message in the transcript, so claude-agent-acp can't fork or roll back at them] → it answers `Fork point message … was not found`, echoed as a failed rollback; the source is untouched.
- [codex-acp's delete and its load of a just-forked thread are unchecked here] → manual check on macOS.
- [A failed delete is only in the tracing log; the source then still lists] → visible in `agent-sessions`; no data lost.
