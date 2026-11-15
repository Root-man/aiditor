## Context

Project grep (`grep.rs`) keeps one `ProjectGrep` in the core: the debounce, the search numbering, streamed hits grouped by `(root, path)`, the status and the regexp used for preview faces. The minibuffer draws any command whose spec has `grep_hits` from it. The grep service walks roots with the index's walker. `*Sessions*` keeps each agent's `session/list` answer in its `SessionList`.

Session files are JSON Lines. A Claude Code line starts with about 400 chars of metadata, and lines reach 100 KB with tool output.

## Goals / Non-Goals

**Goals:**
- Search the listed sessions' files with the existing grep service and prompt, keeping its debounce, streaming, status and cancelling.
- Show readable hits: the text a match falls in, under the session's title.

**Non-Goals:**
- Jumping to the matched message inside the loaded session.
- `CLAUDE_CONFIG_DIR` and `CODEX_HOME` (a `shortcut:` comment).
- Subagent transcripts (`<id>/subagents/*.jsonl`): only the session's own file is searched.
- The agent glyph's own face in the group header: the header keeps `grep-file`.

## Decisions

- **One grep state, two commands.** `ProjectGrep` gains the command it serves and the searched sessions. `poll_grep` serves whichever `grep_hits` command is open instead of `project-grep` alone, and `set_items` gets that command. A second state would duplicate the debounce and the streaming for no gain.
- **File names filter in the service.** `GrepSearch.names`: when not empty, a file is searched only if its name contains one of them. The core knows ids, not paths: Codex's rollout file names carry a date the core can't know. The service is the one already walking, and the filter is one check per file.
- **Roots.** Claude: `<home>/.claude/projects/<cwd>`, `<cwd>` the primary root with non-alphanumerics as `-`, which is how Claude Code names it. Codex: `<home>/.codex/sessions`. A missing root already gives no hits.
- **Text-only filter and snippet in the core, in `on_grepped`.** The service reports a line's first match, which may sit in metadata. The core scans the line's JSON string literals and skips keys and values of a short metadata key list. It unescapes each remaining literal with `serde_json` and runs the search's regexp on the result. The first value that matches becomes the hit's `text` and its `col`, so the long JSON line is not kept. A line with no such value is dropped. The alternative, parsing every agent's JSONL format, would tie the filter to two changing formats.
- **Labels.** A hit's label is `<agent>/<id>:<line>:<snippet>`, unique per line as project grep's is. The header is the glyph and title, found by matching the file name against the searched session ids.
- **Preview** is `Preview::Point` at the session's row in the origin `*Sessions*`, from its `rows`. A folded row has no preview.
- **RET** puts point on the session's row's agent and session and goes through the same code as `agent-sessions-visit`, factored out to take the agent index and session.
- **Key `s`**, the hint `search` after `fold`.

## Risks / Trade-offs

- The metadata key list is a guess at two formats; a new metadata field can bring noise back. → It lives in one const, easy to extend.
- A pattern that only matches the escaped form (say `\"`) finds lines the core then drops, since it matches the unescaped text. → It's rare, and dropping is the safe side.
- The 10,000-hit cap counts lines the core then drops, so a pattern that is common in metadata can fill the cap with lines that are mostly dropped.
- Batches of long lines are copied once before being cut to snippets. → Only up to 64 held hits plus a 50 ms batch at a time.
