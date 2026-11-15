## Why

Frame 14's echo line offers `a archive`, but nothing archives a session yet (#486). Neither adapter advertises `session/archive`, and codex-acp 2.6.0's `session/delete` archives (`thread/archive`), so Codex sessions can be archived now through it; #481 moves to `session/archive` later.

## What Changes

- `a` (`agent-sessions-archive`) on a session row archives it: for an agent named `codex` the core asks the agent service for a fresh list of that agent that sends `session/delete` of the session first (a `shortcut:` until #481); the row leaves every sessions buffer of the project at once and the fresh answer replaces the agent's rows.
- Any other agent fails with `Not supported yet`; elsewhere than a session row, `Select a session row`.
- A session open in the editor has its process stopped first, as `Kill` does; the delete waits until its process group is gone. Its buffer stays, ended.
- The list request gains an optional session to delete, and the agent that runs it to stop, before listing. A delete the agent can't do (no `sessionCapabilities.delete`, an error answer) fails the list with the reason.
- The fake agent's `session/delete` drops the session from its later `session/list` answers.
- The echo line hints gain `a archive`, after pin/unpin.

## Capabilities

### New Capabilities

### Modified Capabilities
- `sessions-list`: archiving with `a`, and its key hint.
- `agent-processes`: a list request that deletes a session first, after stopping its agent.
- `fake-agent`: a deleted session no longer lists.

## Impact

- `crates/aiditor-core/src/sessions.rs`: `agent-sessions-archive`, hint.
- `crates/aiditor-core/src/lib.rs`, `agents.rs`: `AgentOp::List`'s `delete` field.
- `crates/aiditor-core/keymap.toml`, `builtins.rs`: `a`.
- `crates/aiditor-services/src/agents.rs`: delete before listing, waiting for the stopped agent's group.
- `crates/aiditor-services/src/bin/aiditor-fake-agent.rs`: deleted sessions stop listing.
- `docs/editor.md`: Session manager records what #486 built.
