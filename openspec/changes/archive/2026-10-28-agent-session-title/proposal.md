# Proposal

## Why

Every session buffer is named `*<agent>*<<workspace>>`, so a second session in a workspace becomes `*claude*<main><2>` and buffer switching can't tell sessions apart. Claude's and Codex's adapters already send a session title in ACP `session_info_update`, which the editor keeps but ignores.

## What Changes

- A `session_info_update` with a title renames the session buffer to `*<agent>: <title>*<<workspace>>`, for example `*claude: Fruit list*<main>`, made unique as other buffer names are. Only the title's first line is used, cut to 40 characters.
- A `session_info_update` with a null title (ACP's "clear") renames the buffer back to `*<agent>*<<workspace>>`. One without a title, such as Codex's `_meta`-only thread status updates, leaves the name alone.
- Each new title renames again, so Codex's prompt-as-title, followed by its summary title, renames twice.
- Session buffer names belong to the agent. No command renames them by hand, and the spec records that a future `rename-buffer` must refuse session buffers. Renaming the workspace does not change them.
- `docs/editor.md` records the behaviour: the session buffer paragraph (built in #292) and the session manager's `title`.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `session-buffers`: the session buffer's name follows the agent's session title and can't be renamed by hand.

## Impact

- `crates/aiditor-core/src/agents.rs`: `agent_update` handles `SessionUpdate::SessionInfoUpdate`. The name is built where `agent_start` builds it now. There is a new unit test.
- `crates/aiditor-core/src/buffer.rs`: reuses `Buffers::rename` and `unique` without changes.
- `docs/editor.md`: updates the session buffer and session manager sections.
- The ACP schema is unchanged: `SessionInfoUpdate` is in `agent-client-protocol-schema` 1.12.0 without an unstable feature.
