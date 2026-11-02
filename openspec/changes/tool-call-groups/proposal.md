# Proposal

## Why

An agent working through a task makes dozens of calls, each a header and a blank line, so the messages that matter scroll away. Frame 6 and #417 fold consecutive calls into one line that says what the agent did and whether anything failed.

## What Changes

- Consecutive tool calls fold into a group line, `▸ <icon> <verbs>`, with `<n> failed` and the group's duration on the right. A single call stays as it is.
- `TAB` opens the group into rows, and a row into its call; `TAB` inside folds back to the level above.
- Thoughts and answered permission lines in a run of calls become rows. A thought row is one line, `▸ Thought for 4s`, its text folded under it.
- A call waiting for permission keeps its row and block showing under a folded group until answered.
- Replayed history groups the same way, without durations.
- **BREAKING** (display only): rows in a group sit two columns in, and the blank lines between them go.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `session-buffers`: a new requirement group for tool groups, and `TAB` reaching groups and thought rows.

## Impact

`crates/aiditor-core/src/agents.rs` and `agents/render.rs`, service tests that send two calls in a row, and `docs/editor.md`. No new dependency, face or icon. Agent questions (`elicitation/create`) aren't built, so "a question's block shows in place of the row" waits for them.
