# Design

## Context

The agent service (`aiditor-services/src/agents.rs`) spawns one long-lived process per session and answers each `AgentOp` on a per-agent channel that `run` returns. #102 decided that listing uses a throwaway process per agent; #323 will ask every registry agent at once and drop late answers.

## Goals / Non-Goals

**Goals:** one op that lists one agent's sessions in `cwd` through every page and leaves no process behind.

**Non-Goals:** the sessions buffer and the core's handling of the answer (#323); on-demand paging (the user chose fetch-all after measuring both adapters).

## Decisions

- **The request number is the only identity.** `AgentOp::List { request: u64, .. }` → `Msg::AgentListed { request, result: Result<Vec<SessionInfo>, String> }`, like `ConfigurationLoaded { request }`. The core maps numbers to agent names; no `AgentId`, since nothing else addresses the process.
- **`run` returns a receiver for the one answer**, so the editor's effect loop drains it as for `Spawn` and needs no change.
- **Stderr is discarded** (`Stdio::null`): there is no log buffer for a throwaway process. The error is the ACP error's message.
- **Answer first, then terminate.** The answer goes out as soon as the last page arrives; the task then SIGTERMs the group (unless exited) and SIGKILLs it after the grace, with the existing `terminate` and `kill_after_grace`.
- **Quit kills lists too.** Running lists' pids and exit flags sit in a shared map keyed by request; the task removes its entry before terminating, and `kill_all` drains the rest into the same grace.
- **Page limit 50** with a `shortcut:` comment; the sessions so far are answered when it's hit.
- **No timeout.** An agent that never answers keeps its process until quit; a superseding `g` starts another. Noted as a `shortcut:`.
- **Fake agent fixture**: `sessions: Option<Vec<Vec<SessionInfo>>>`; cursor is the page index as text; filtering by `cwd` lets the test check the `cwd` sent, since stderr is discarded. The test gets the pid by wrapping the fake in `sh -c 'echo $$ > file; exec ...'`.

## Risks / Trade-offs

- A hung agent leaks a process until quit (no timeout).
- Group id reuse between the exit check and SIGKILL, as already accepted for sessions.
