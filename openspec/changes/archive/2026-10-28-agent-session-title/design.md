# Design

## Context

`agent_start` (`crates/aiditor-core/src/agents.rs`) names the session buffer `*{name}*<{workspace}>` and keeps the agent's registry name in `Session::agent`. It doesn't keep the workspace name. `agent_update` stores every `SessionNotification` in `session.updates` and handles only some variants; `SessionUpdate::SessionInfoUpdate` falls through. `SessionInfoUpdate::title` is a `MaybeUndefined<String>`: undefined (field absent), null (clear) or a value. `Buffers::rename` already applies `unique`, skipping the buffer being renamed.

## Goals / Non-Goals

**Goals:**
- Handle the title in `agent_update` next to the `ConfigOptionUpdate` branch, using the existing rename path.

**Non-Goals:**
- Renaming the stderr log buffer `*<agent> stderr*<<workspace>>`. It is only for debugging, and nobody looks for it by title.
- Showing `updatedAt` or storing the title in a session index. The session manager doesn't exist yet.
- A `rename-buffer` command. The spec only says such a command must refuse session buffers.

## Decisions

- **Keep the workspace name in `Session`.** Add `workspace: String`, set in `agent_start`. The workspace's current name is not used, because the spec says a workspace rename doesn't touch session buffers. Building the name from the buffer's current name was rejected: parsing `*…*<…>` back out breaks on titles that contain `*` or `<`.
- **One function builds the name, `fn session_name(agent, workspace, title: Option<&str>)`.** It is used both by `agent_start` (with `None`) and by the title update, so both produce the same format. It trims the first line and cuts at 40 chars, not bytes, using `chars().take(40)`, so a cut never splits a UTF-8 char.
- **Null and empty both mean no title.** Both go back to the base name, so a buffer never gets a `*claude: *<main>` name.
- **Renaming only touches the buffer's name.** Windows, `recent` and the mode line all refer to buffers by `BufferId`, so the frame shows the new name on the next build. Nothing else has to change.

## Risks / Trade-offs

- [Codex sends the whole prompt as its first title] → The first line is cut to 40 chars, and its summary title replaces it moments later.
- [A rename moves the buffer in name-sorted completion lists while the user is reading them] → This is acceptable: the agent sends a title once or twice per session.
- [Two sessions with one title get `<2>`, which can move from one to the other as titles change] → This is the same `unique` rule file buffers use.
