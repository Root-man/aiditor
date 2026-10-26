## Why

Tool calls are the bulk of an agent's turn, but the session buffer keeps `tool_call` and `tool_call_update` unrendered, so a turn shows only the agent's prose (#294, epic M3 session buffer). Each call needs to be visible at a glance, with its full input and output one key away instead of hidden behind a "Ran command" summary.

## What Changes

- Each `tool_call` adds a block above the prompt line: a header line `<title> (<kind>, <status>)` with a rule beside it, and its content folded under it as invisible text shown as ` …`.
- The content is everything the agent sent: `Input` with the call's whole `rawInput` as JSON, then each content item (text as markdown, a diff as a unified diff, a terminal by id), then `Output` with the whole `rawOutput` when no content item carries the result. Nothing is cut.
- `tool_call_update` merges into the call and rewrites its block in place, wherever it sits in the transcript, keeping its fold state; an update for an unknown call with a title starts a block.
- `TAB` on a tool call's header (`agent-toggle-tool-call`) shows or hides its content, as in magit; elsewhere it echoes `No tool call here`. An unfolded call's content is rendered as markdown with the rule beside every line.
- `C-n`/`C-p` and scrolling step over folded text as one line, so a folded call is one line to move past.

## Capabilities

### New Capabilities

### Modified Capabilities
- `session-buffers`: tool calls render as folded blocks that update in place and toggle with `TAB`.
- `motion`: line motion and scrolling treat folded lines as hidden.

## Impact

`aiditor-core`: `agents.rs` (tool blocks, the toggle command), `buffer.rs` (in-place output replacement, a fold lookup, properties kept in order when added mid-buffer), `motion.rs` and `window.rs` (skipping folds), `builtins.rs` and `keymap.toml` (`TAB` in `agent-session-mode`); `docs/editor.md`. The frontend needs no change: rules and folds already draw.
