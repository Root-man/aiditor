## Why

`C-s` in `*Sessions*` only searches the list's titles. Finding the session where something was said means opening sessions one by one (#487). The grep service already searches files fast and streams hits to a live prompt, so the session files can be searched the same way.

## What Changes

- `s` (`agent-sessions-search`) in a sessions buffer prompts `Search sessions in <project>: ` and works like `project-grep`: it searches after a 200 ms pause once 3 chars are typed, hits stream in, the list shows `[searching]` and `[no matches]`, it has its own `M-p` history and `C-g` cancels.
- It searches the agents' own files, limited to the sessions listed for the current project:
  - Claude Code's `~/.claude/projects/<cwd>/<id>.jsonl`, `<cwd>` being the primary root with every char other than ASCII letters and digits turned into `-`;
  - Codex's `~/.codex/sessions/**/rollout-*-<id>.jsonl`.
  - Agents registered under other names are not searched.
- A match counts only inside a JSON string value that holds text. Keys and metadata values (ids, `cwd`, timestamps, signatures and the like) are not hits.
- Hits are grouped under the session's agent glyph and title. Each hit shows the unescaped text of the string it falls in, newlines as spaces, starting a little before the match. No line numbers are shown.
- The selected hit is previewed by moving point to its session's row in `*Sessions*`. `RET` opens or loads the session as `RET` on its row does.
- The grep service's search gains file names: when some are given, a file is searched only if its name contains one of them.
- The echo line hints gain `s search`, after fold.

## Capabilities

### New Capabilities
- `session-search`: searching the text of the listed sessions from `*Sessions*`.

### Modified Capabilities
- `project-grep`: a search can be limited to files whose names contain given strings.
- `sessions-list`: the `s search` key hint.

## Impact

- `crates/aiditor-core/src/grep.rs`: the grep prompt state also serves the session search: the command it belongs to, the searched sessions, the text-only filter and snippets, header labels and the session preview.
- `crates/aiditor-core/src/sessions.rs`: `agent-sessions-search`, its accept handler, the hint.
- `crates/aiditor-core/src/minibuffer.rs`: session hits are listed without line numbers.
- `crates/aiditor-core/src/lib.rs`: `GrepSearch.names`.
- `crates/aiditor-core/src/builtins.rs`, `keymap.toml`: the command and `s`.
- `crates/aiditor-services/src/grep.rs`: the file-name filter.
- `docs/editor.md`: the Session manager section settles how the hits are shown.
