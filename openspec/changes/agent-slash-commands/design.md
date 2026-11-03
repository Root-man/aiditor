# Design

## Context

ACP sends `available_commands_update` with the full list each time (name, description, optional `input.hint`); a prompt that starts with `/name` runs it, so the client only needs to help type it. claude-agent-acp already leaves out commands the CLI handles interactively (`/clear`, `/login`…); codex-acp sends `plan`, which turns plan mode on, while the editor switches modes with `S-TAB`. Minibuffer prompts take candidates and per-candidate annotations from the command's `ArgSpec`, worked out when the prompt opens from the window it was opened in; a prompt whose answer matches nothing returns the typed text.

## Goals / Non-Goals

**Goals:** pick a session's command by name with its hint and description in view, and insert it into the prompt.

**Non-Goals:** completing a command's arguments; a configurable hidden list; showing commands in the transcript or mode line.

## Decisions

- **Stored on the session.** `Session.commands` holds the latest list minus a fixed `HIDDEN` list (`plan`), replaced on each update. Alternative: filter when the prompt opens. Rejected: one place either way, and storing filtered keeps the prompt code free of it.
- **`agent-command` is an ordinary prompting command.** `ArgSpec { prompts: ["Command: "], completion, annotation }`; completion and annotation look up the session whose buffer is current. The annotation is the hint, two spaces and the description's first non-blank line, cut to 60 chars with `…`, as Claude's skill descriptions run to paragraphs. The handler inserts the command's chip and a space at point in the prompt area, or at the prompt's end from the transcript (a space first when that end follows text); an answer naming no command goes in as `/answer ` text. It fails outside a session buffer.
- **Anywhere in the message.** Both CLIs take commands anywhere in a message, so `/` opens the prompt at a word's start in the prompt area (its start or after whitespace), not only at the prompt's start; inside a word, as in a path, it types itself.
- **Command chips (review).** A picked command is a chip like the file chips (`Kind::Command`, interned by name, no path), so it reads apart from the text and is deleted as one char, with drawing, repainting and word motion already built. It draws ` ▷ /name `: `▷` for run, outlined so it isn't the filled `▸` fold marker. `blocks` puts it back as `/name` inside the text run, so the prompt goes as text. `RET` on it sends rather than opening a file. Replayed and echoed user text turns each `/name` at a word's start into a chip when the session's list names it. Alternative: a face on editable `/name` text. Rejected: its own scan and repaint, and not atomic.
- **`/` is `agent-slash` in `agent-session-mode`.** With a non-empty command list and point at a word's start in the prompt area it calls `agent-command`; otherwise it runs `self-insert-command`. `C-g` leaves the prompt untouched.

## Risks / Trade-offs

- [The hidden list is by name across agents] → a Claude command named `plan` would be hidden too; Claude has none today.
- [A literal `/` at a word's start, as in ` /usr/bin`, needs the picker] → answer it with `usr/bin` (inserted with a trailing space) or `C-g` and type it after other text; no `quoted-insert` exists yet.
- [Replay knows only listed commands] → if the list arrives after a `session/load` replay, replayed commands stay text.
