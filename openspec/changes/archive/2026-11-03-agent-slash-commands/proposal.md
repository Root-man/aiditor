# Proposal

## Why

Claude Code and Codex both offer slash commands (skills, `/review`, `/compact`, `/mcp`…) and announce them with ACP's `available_commands_update`. The session buffer has no way to find or pick them (issue #373, M6).

## What Changes

- The core keeps each session's latest `available_commands_update` list, each update replacing the last.
- Commands the editor does itself are left out of the list: Codex's `plan`, as `S-TAB` cycles modes.
- `/` at a word's start in the prompt area opens `Command: ` over the session's commands, each annotated with its argument hint and description; with no commands, or inside a word, `/` inserts itself.
- `M-x agent-command` opens the same prompt from anywhere in a session buffer.
- Picking a command inserts a command chip at point, drawn ` ▷ /name ` like the file chips so commands stand apart from the text; from the transcript it goes at the prompt's end. An answer that names no command goes in as `/answer ` text. The prompt still goes as text, as ACP expects.
- Sent and replayed user messages show the session's known `/name`s as command chips.

Not in scope: argument completion for a command, showing the commands anywhere else, a per-agent hidden list in settings.

## Capabilities

### New Capabilities

### Modified Capabilities
- `session-buffers`: the session's command list, the `Command:` prompt, `/`, `agent-command` and command chips.

## Impact

- `crates/aiditor-core/src/agents.rs`, `agents/chips.rs`: store the list, the prompt's candidates and annotations, the command chip, insertion, replay.
- `crates/aiditor-core/src/builtins.rs`, `keymap.toml`: `agent-command`, `agent-slash` and the `/` binding.
- `docs/editor.md`: the ACP section records the behaviour.
