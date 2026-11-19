# Proposal

## Why

A session that asks for permission, asks a question, ends its turn or dies waits silently while you look at another app or another workspace. #524 laid the notification plumbing; this change makes session events use it (#525).

## What Changes

- A session posts a system notification, tagged with its session buffer and titled `<agent> · <workspace>`, when it:
  - asks for permission: the body is the tool call's title;
  - asks a blocking question: the body is the question;
  - ends a turn: `Turn ended`, or the session's own line (`Turn ended: max_tokens`, `Error: …`) when it didn't end with `end_turn`; a cancelled turn posts nothing;
  - fails to start or exits on its own: the line `*Messages*` or the echo area gets.
- Only while you can't see the session: the window is inactive, or the current workspace's windows don't show its buffer. Seeing it later withdraws nothing.

Not included: non-blocking questions (they don't make the session `Asking`; the turn's end notifies), URL requests (declined until questions can show them), terminal events (#526), Linux (#527).

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `system-notifications`: agent session events notify while the session can't be seen.

## Impact

- `crates/aiditor-core`: `agents.rs` and `agents/asks.rs` post on the four events; `notifications.rs` gains the visibility check; tests in `agents/tests.rs`.
- `docs/editor.md`: the notifications entry records what was built.
