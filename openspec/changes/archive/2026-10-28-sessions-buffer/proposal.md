# Proposal

## Why

The agent service can list an agent's sessions (#322), but nothing in the editor asks it or shows the answers. `agent-sessions` (#323) puts every registry agent's past sessions in the current project into one buffer, so a session started earlier, in the editor or in the CLI or desktop app, can be found and, once #103 lands, continued.

## What Changes

- New M-x command `agent-sessions`: sends one `AgentOp::List` per registry agent, with the current workspace's project primary root as `cwd` and the project's environment. Like an agent spawn, it waits while the project's environment is being captured.
- It shows `*Sessions*<<workspace>>`: read-only, owned by the workspace, in `agent-sessions-mode`, reused by a later `agent-sessions` in that workspace. Rows show the agent, the last update in the local time zone, `[open]` for a session open in the editor, and the title. Rows are sorted by `updatedAt`, most recent first, across agents; a missing or unparseable time sorts last and shows blank.
- An agent that hasn't answered shows `Listing <agent>...`; an agent's error shows in place of its rows. Each answer rebuilds the list, keeping point on the same session.
- `g` asks every agent again and drops answers to the previous request; `n`/`p` move between rows; `q` quits; `RET` on an `[open]` row switches to its session buffer and on other rows says loading is not supported yet (#103).
- `aiditor-core` takes `jiff` to parse `updatedAt` as RFC 3339 with any offset or fraction and show it in local time.
- Not doing: per-session live state marks (`[working]`, `[idle]`...) the spec names; the issue asks only for `[open]`. Loading a session (#103).

## Capabilities

### New Capabilities
- `sessions-list`: the `agent-sessions` command and its `*Sessions*` buffer.

### Modified Capabilities

## Impact

`aiditor-core`: new `sessions.rs`, `Msg::AgentListed` handling, `release_agents` also releasing held lists, builtins, `keymap.toml`, `Cargo.toml` (`jiff`). `docs/editor.md` session manager notes.
