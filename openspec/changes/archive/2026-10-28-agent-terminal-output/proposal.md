# Proposal

## Why

Agent shell commands show only `Terminal <id>` in their tool call: both adapters send the command's output through the `_meta` terminal extension (decided in #100), which the editor ignores. Issue #316 shows that output where the command ran.

## What Changes

- `initialize` advertises `clientCapabilities._meta.terminal_output: true`, so claude-agent-acp sends its output through the extension instead of a text block.
- The session buffer collects output per terminal id: a `tool_call` with `_meta.terminal_info` or a `terminal` content item opens the terminal; every `data` under `_meta.terminal_output` or `_meta.terminal_output_delta` appends to it; output for a terminal no call announced is dropped.
- The call's content shows that output, ANSI escapes and carriage returns stripped, as `Output:` in a fenced block in place of the `Terminal <id>` line; nothing shows before the first chunk.
- `_meta.terminal_exit` with a non-zero exit code or a signal adds `exit <code>` or `signal <name>` to the header.
- Updates that only carry output re-render their call when the frame is built, so a fast command re-renders it at most once per frame.
- The fake agent also reports the advertised `terminal_output` capability.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `session-buffers`: tool call content shows terminal output; the header shows a failed command's exit; output-only updates render per frame.
- `agent-processes`: `initialize` advertises `_meta.terminal_output`.
- `fake-agent`: reports the advertised `terminal_output` capability.

## Impact

- `crates/aiditor-core/src/agents.rs`: terminal state per tool call, rendering, per-frame re-render.
- `crates/aiditor-services/src/agents.rs`: the advertised capability.
- `crates/aiditor-services/src/bin/aiditor-fake-agent.rs`, `tests/agents.rs`: capability report and a fake-agent test of streamed output.
- `docs/editor.md`: built note under Agent command output; fake agent report line.
