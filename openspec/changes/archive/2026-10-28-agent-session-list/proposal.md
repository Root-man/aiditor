# Proposal

## Why

`agent-sessions` (#323) lists a project's past sessions from the agents themselves (#102): each agent is asked for `session/list` by a throwaway process, so a list never adds a long-lived process. The agent service has no way to do that yet.

## What Changes

- New `AgentOp::List { request, command, args, cwd, env }`: spawns the agent like `Spawn`, sends `initialize`, and when the agent advertises `sessionCapabilities.list` sends `session/list` with `cwd`, following `nextCursor` for up to 50 pages. It then terminates the process and answers `Msg::AgentListed { request, result }` with the sessions or an error (spawn failure, no list capability, an error answer, the agent exiting).
- `request` is chosen by the core and echoed back, so it can drop answers to a superseded request. The core ignores `AgentListed` until #323.
- List processes are killed on quit with the agents.
- The fake agent's script takes an optional `sessions` fixture: a list of pages. With it, the fake advertises `sessionCapabilities.list` and answers `session/list` with the sessions of the page the cursor names that have the request's `cwd`.
- Not doing: loading pages on demand as the list scrolls (measured: claude-agent-acp answers in one page, codex-acp's pages are cheap after the first).

## Capabilities

### New Capabilities

### Modified Capabilities
- `agent-processes`: adds listing an agent's sessions in a throwaway process, and list processes on quit.
- `fake-agent`: adds the `sessions` fixture and `session/list`.

## Impact

`aiditor-core` (`AgentOp`, `Msg`), `aiditor-services` agent service and fake agent, the editor's effect loop (unchanged: `run` still returns a receiver), `docs/editor.md`.
