# Design

## Context

ACP sends `available_commands_update` with the full list each time (name, description, optional `input.hint`); a prompt that starts with `/name` runs it, so the client only needs to help type it. claude-agent-acp already leaves out commands the CLI handles interactively (`/clear`, `/login`…); codex-acp sends `plan`, which turns plan mode on, while the editor switches modes with `S-TAB`. Minibuffer prompts take candidates and per-candidate annotations from the command's `ArgSpec`, worked out when the prompt opens from the window it was opened in; a prompt whose answer matches nothing returns the typed text.

## Goals / Non-Goals

**Goals:** pick a session's command by name with its hint and description in view, and insert it into the prompt.

**Non-Goals:** completing a command's arguments; a configurable hidden list; showing commands in the transcript or mode line.

## Decisions

- **Stored on the session.** `Session.commands` holds the latest list minus a fixed `HIDDEN` list (`plan`), replaced on each update. Alternative: filter when the prompt opens. Rejected: one place either way, and storing filtered keeps the prompt code free of it.
- **`agent-command` is an ordinary prompting command.** `ArgSpec { prompts: ["Command: "], completion, annotation }`; completion and annotation look up the session whose buffer is current. The annotation is the hint, two spaces and the description's first non-blank line, cut to 60 chars with `…`, as Claude's skill descriptions run to paragraphs. The handler inserts `/name ` at the prompt area's start and moves point after it. It fails outside a session buffer.
- **`/` is `agent-slash` in `agent-session-mode`.** With point at the prompt area's start and a non-empty command list it calls `agent-command`; otherwise it runs `self-insert-command`, so `/` elsewhere and in a session without commands types as before. `C-g` leaves the prompt untouched; typing a name that matches nothing and `RET` inserts `/that `.

## Risks / Trade-offs

- [The hidden list is by name across agents] → a Claude command named `plan` would be hidden too; Claude has none today.
- [A literal `/` at the prompt's start needs the picker] → type the text after, or answer the prompt with the text; no `quoted-insert` exists yet.
