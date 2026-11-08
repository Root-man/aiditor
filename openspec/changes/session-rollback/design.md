# Design

## Context

Fork (#349) spawns a hidden session buffer, sends `session/fork` (cut at a message through `_meta.jetbrains.air.fork`) then `session/load`, and on the load's answer shows the buffer if the source is still selected; a failed fork kills its buffers. Rollback reuses all of that and changes only what happens at the answer. Both adapters advertise `sessionCapabilities.delete`: claude-agent-acp 0.97's `session/delete` tears down a loaded session and removes its stored transcript; codex-acp 2.2.2 advertises it (checked by `initialize` on macOS), its effect on disk not checked.

## Goals / Non-Goals

**Goals:** rollback through the fork path with one new `Open` variant and one session flag.

**Non-Goals:** a key binding (M-x only, like fork); reverting files; rolling back from `*Sessions*`; reporting a failed delete beyond the log.

## Decisions

- **Fork first, swap on success.** The source stays usable until the fork has loaded, so a failure loses nothing. Alternative, clearing the source buffer and loading into it, loses the transcript on failure.
- **`Open::Rollback(SessionId, MessageId)`.** The service handles it as `Open::Fork(source, Some(at))` and then deletes the source; the core reads it to set `Session::rollback`, so `start_session`'s signature stays. The fork's `load` holds the source buffer as `from` already, since the command runs in the selected window's session buffer, so no buffer id is stored.
- **The swap**, in `agent_started`'s success arm: each window of `core.windows` and of every inactive workspace's windows that shows the source shows the fork; then the source buffer and its log are killed (`sync_agents` cancels its asks and kills its process) and the fork's buffer and log are renamed to the freed names. If the source buffer is already gone nothing is swapped: the fork keeps its own name, unshown.
- **Delete through the fork's process, inside `connect`,** after the load answers and before `AgentStarted`, so it needs no new `AgentOp` and the service keeps no capabilities. The source process is still alive then but idle unless a turn runs; claude-agent-acp's delete in a process that hasn't loaded the source only removes the file. A failed delete is a `tracing::warn`.
- **The message at point** is found by one helper shared with `agent-fork-session`; each command words its own refusals. The prompt line counts as no message for rollback (fork takes it as "the whole session").
- **The question** is asked through `read_string` with `no`/`yes`, as `kill-buffer` does; the answer re-runs the command with point unchanged.

## Risks / Trade-offs

- [A turn running in the source when the rollback swaps could still write to its transcript while the process is being killed, after the delete] → accepted; the confirmation stands between, and a leftover transcript just lists.
- [codex-acp's delete and its load of a just-forked thread are unchecked here] → manual check on macOS.
- [A failed delete is only in the tracing log; the source then still lists] → visible in `agent-sessions`; no data lost.
