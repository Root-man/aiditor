# Proposal

## Why

Agents name the files they read, edit and talk about, but the session buffer shows those names as plain text, so getting to one means retyping it in `find-file`. Issue #367 makes them links.

## What Changes

- The path a tool call's header shows (its ACP location, or its diff's path) is drawn in the `link` face when the file exists under a project root.
- `path`, `path:line` and `path:line:col` references in agent messages are drawn in the `link` face when the path exists under a project root, relative to any root or absolute.
- A path that doesn't exist stays plain text.
- `RET` on a link visits the file, at the line when there is one, in another window. `RET` elsewhere on a tool call's header still opens its full output.

## Capabilities

### New Capabilities

### Modified Capabilities
- `session-buffers`: file links in agent messages and tool-call headers, and `RET` on a link; `RET` on a header's link visits the file instead of opening the output.

## Impact

- `crates/aiditor-core/src/markdown.rs`: path tokens get the `link` face when a caller-supplied test says the file exists.
- `crates/aiditor-core/src/agents.rs`, `agents/render.rs`: links against the project's file index, `RET` on a link.
- `crates/aiditor-core/src/grep.rs`: the visit-at-line code is shared with project grep.
- `docs/editor.md`: the ACP section records the behaviour.
