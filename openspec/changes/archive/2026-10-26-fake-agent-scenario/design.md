# Design

## Context

`tests/agents.rs` has `Live`, a core driven by keys over the real agent service with the fake agent; `tests/file_sync.rs` has `Editor`, a core over the real file service and native watcher on a temporary project. The scenario needs both at once. `agent-start` spawns the agent in the workspace project's primary root, so the fake's `write` step lands in the project.

## Goals / Non-Goals

**Goals:** one scenario test covering the issue's three assertions on macOS and Linux.

**Non-Goals:** a shared harness for future scenarios, the GPUI frontend, permissions, terminals.

## Decisions

- **A new test file, its own small driver.** The scenario combines both harnesses' effect routing (files, watch sets, agents); folding it into `agents.rs` or `file_sync.rs` would widen a harness the other tests don't need. A shared support module is deferred until a second scenario exists.
- **One loop over both event sources.** The watcher's callback forwards into a Tokio unbounded channel, and the driver `select!`s it with the agent's receiver, feeding each message to the core until a condition on the core holds. Polling a std channel from async code was the alternative; `select!` avoids busy waits.
- **The written file is read into a clean buffer before prompting, and the watcher is confirmed ready** the way `file_sync.rs` does, by rewriting the file until its change is reported, so the agent's later write isn't lost to watcher start-up.
- **No process left** is checked as `agents.rs` does: the agent's `pid=` stderr line, then `kill(pid, 0)` failing after the agent channel closes.

## Risks / Trade-offs

- [Native watcher latency on CI] → the same 10 s deadlines as `file_sync.rs`.
