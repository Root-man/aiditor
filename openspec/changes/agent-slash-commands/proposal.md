# Proposal

## Why

Claude Code and Codex both offer slash commands (skills, `/review`, `/compact`, `/mcp`…) and announce them with ACP's `available_commands_update`. The session buffer has no way to find or pick them (issue #373, M6).

## What Changes

- The core keeps each session's latest `available_commands_update` list, each update replacing the last.
- Commands the editor does itself are left out of the list: Codex's `plan`, as `S-TAB` cycles modes.
- `/` with point at the start of the prompt area opens `Command: ` over the session's commands, each annotated with its argument hint and description; with no commands, or elsewhere, `/` inserts itself.
- `M-x agent-command` opens the same prompt from anywhere in a session buffer.
- Picking a command inserts `/name ` at the start of the prompt area and leaves point after it; an answer that names no command is inserted the same way. The prompt still goes as text, as ACP expects.

Not in scope: argument completion for a command, showing the commands anywhere else, a per-agent hidden list in settings.

## Capabilities

### New Capabilities

### Modified Capabilities
- `session-buffers`: the session's command list, the `Command:` prompt, `/` and `agent-command`.

## Impact

- `crates/aiditor-core/src/agents.rs`: store the list, the prompt's candidates and annotations, insertion.
- `crates/aiditor-core/src/builtins.rs`, `keymap.toml`: `agent-command`, `agent-slash` and the `/` binding.
- `docs/editor.md`: the ACP section records the behaviour.
