# Proposal

## Why

Agent command output shows with its ANSI escapes stripped (#316), so `cargo test` and `git diff` output loses its colours. Issue #317 keeps them as faces.

## What Changes

- SGR escapes in a call's terminal output, and in its text content items (where Codex sends a read or list command's output), become faces: foreground and background in the theme's 16 ANSI colours, bold, underline, and their resets. Every other escape, and carriage returns, are still dropped.
- The escapes stay out of the buffer's text, so copying output yields plain text.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `session-buffers`: command output keeps its SGR colours, bold and underline as faces.

## Impact

- `crates/aiditor-core/src/agents.rs`: the stripping pass also returns faces; the tool call keeps them and adds them when it renders open.
- `docs/editor.md`: built note under Agent command output.
