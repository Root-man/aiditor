# Proposal

## Why

#370 adds the project reference picker promised by the interactive prompt design. Chips already render, edit and send, but typing `@` cannot yet create one.

## What Changes

- Typing `@` in a session's editable prompt inserts it and opens `Reference in <project>: ` over indexed files and directories from every root.
- Picking a candidate replaces that `@` with its chip; cancelling keeps the literal `@` and surrounding text.
- Reuse project indexing, captured root labels, fuzzy completion and existing chip behavior, without previewing over the session.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `session-buffers`: Project-scoped reference completion and insertion into the prompt.

## Impact

- Core agent input handler, command registration and session keymap.
- Existing project finder candidate construction and delayed index completion integration.
- Harness tests, `docs/editor.md`, and OpenSpec artifacts. No dependencies, service/protocol changes, or general filesystem attachment picker (#371).
