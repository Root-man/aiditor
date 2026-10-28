# Proposal

## Why

The sessions list marks sessions open in the editor only with `[open]`, so it can't tell a session that's busy, waiting for a permission answer or dead from one ready for a prompt. The core already knows each live session's state without ACP; `session/list` reports none (issue #324).

## What Changes

- Rows of sessions open in the editor show their state instead of `[open]`: `[starting]` before `session/new` answers, `[working]` while a turn runs, `[waiting]` while a permission request is pending, `[idle]` between turns, `[ended]` once the agent process exited or failed to start.
- A sessions buffer rebuilds itself when a row's state changes, without `g`, keeping point on its session.
- Sessions not open in the editor still show no state.

## Capabilities

### New Capabilities

### Modified Capabilities
- `sessions-list`: rows show the open session's state instead of `[open]`, and the list follows state changes; `RET` refers to rows with a state.

## Impact

`crates/aiditor-core/src/sessions.rs` (row label, re-render on change), `crates/aiditor-core/src/agents.rs` (state of an open session), `crates/aiditor-core/src/lib.rs` (post-update check). No protocol, dependency or frontend changes.
