# Proposal

## Why

ACP carries no times, so a session buffer can't say when anything happened, how long a tool ran or how long the agent has been working. Frame 6 of the visual direction shows these; issue #364 builds them.

## What Changes

- Live user and agent messages show their arrival clock time right-aligned in `annotation` on their first line.
- A live thought gets a header line, `Thought for 4s`.
- A tool call's header shows how long it ran, right-aligned, and `waiting 0:12`, ticking, while a permission request for it is open.
- While a turn runs, the transcript's last line above the prompt reads `Working for 2m 16s`, ticking each second and counting permission waits. When the turn ends, however it ended, the same line becomes `Worked 2m 16s` with the clock time beside it.
- The ticking timer runs only while a session with something ticking is shown, and a session's lines refresh when it is shown.
- History replayed by `session/load` shows no times.
- The frontend passes its minute clock to the core, which never reads the wall clock.

## Capabilities

### New Capabilities

### Modified Capabilities
- `session-buffers`: times on messages, thoughts and tool calls, the waiting counter, the working line.
- `frontend-rendering`: the workspace clock's text also reaches the core.

## Impact

`crates/aiditor-core/src/agents.rs`, `agents/render.rs`, `frame.rs`, `lib.rs` (`Msg::Clock`, the last `now`, the tick timer), `crates/aiditor/src/editor.rs` (`start_clock`), tests that assert session text, `docs/editor.md`.
