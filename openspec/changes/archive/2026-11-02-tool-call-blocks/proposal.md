# Proposal

## Why

A tool call's header still reads `<title> (<kind>, <status>)` and its open content is raw JSON and the whole output, so a transcript of agent work is noisy and a long command buries everything under it. Frame 6 and #365 want a one-line header and a bounded open view, with the full output a key away. Tool groups and file links build on this header.

## What Changes

- **BREAKING** (display only): the header becomes `<fold> <status> <icon> <verb> <target>` with the duration or exit status right-aligned; the ACP names and the ` …` placeholder go.
- A command's open body is `$ <command>` and its first 20 output lines, then `… N more lines`. The rest stays in the buffer's text, hidden, so search and copy still reach it.
- An edit's open body is its coloured diff, without the `Input:` JSON.
- `RET` on a header opens the call's full output in a read-only buffer of its own.
- Other calls keep today's `Input:` and output body, uncut.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `session-buffers`: tool call headers, open bodies, `TAB`, where a failed command's exit shows, and a new `RET` on a header.

## Impact

`crates/aiditor-core/src/agents.rs` and `agents/render.rs`, the service tests that read headers, and `docs/editor.md`. No new dependency, face or icon: `Prop::Icon`, `AlignRight` and the `diff-*` faces exist already.
