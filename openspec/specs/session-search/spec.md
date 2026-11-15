# session-search Specification

## Purpose
Searches the whole text of the sessions `*Sessions*` lists, from the agents' own session files, in a live prompt that works like project grep.

## Requirements

### Requirement: s searches the listed sessions
`s` (`agent-sessions-search`) in a sessions buffer SHALL prompt `Search sessions in <project>: ` and search as `project-grep` does: after the debounce once 3 chars are typed, streaming hits, with the same status, history and cancelling. Outside a sessions buffer it SHALL fail with `Not in a sessions buffer`; with no searchable session listed, with `No sessions to search`.

#### Scenario: Prompt and search
- **WHEN** `s` is pressed in `*Sessions*` of project `app` and `needle` is typed
- **THEN** the prompt reads `Search sessions in app: ` and, after the pause, one grep search for `needle` starts

#### Scenario: Elsewhere
- **WHEN** `agent-sessions-search` runs in `*scratch*`
- **THEN** it fails with `Not in a sessions buffer`

### Requirement: Only the listed sessions' files are searched
The search SHALL cover, for each listed session of an agent registered as `claude`, `~/.claude/projects/<cwd>/<id>.jsonl`, `<cwd>` being the list's primary root with every char other than an ASCII letter or digit turned into `-`, and for one registered as `codex`, the files under `~/.codex/sessions` whose names contain its id. Other agents' sessions and every unlisted session SHALL NOT be searched.

#### Scenario: Roots and names
- **WHEN** `/r/my.app` lists `claude` session `c1` and `codex` session `x1`, and a search starts
- **THEN** it searches `~/.claude/projects/-r-my-app` and `~/.codex/sessions`, limited to file names containing `c1` or `x1`

### Requirement: Only text is a hit
A hit SHALL be kept only when the pattern matches inside a JSON string value of its line that is not a key and whose key is not metadata (ids, `cwd`, timestamps, versions, signatures, encrypted content, base instructions). The first such value SHALL be the hit's text, unescaped, with line breaks as spaces, starting up to 20 chars before its first match, `…` marking a cut.

#### Scenario: A word only in text
- **WHEN** a hit's line is `{"cwd":"/r/app","message":{"content":"fix the app now"}}` and the pattern is `app`
- **THEN** the hit shows `fix the app now`, with its match at column 8

#### Scenario: A word only in metadata
- **WHEN** a hit's line is `{"cwd":"/r/app","message":{"content":"hello"}}` and the pattern is `app`
- **THEN** the hit is not listed

### Requirement: Hits are grouped by session
Hits SHALL be grouped as project grep's are, each group's header being the session's agent glyph and title, `(untitled)` without one, and each hit showing its text without a line number.

#### Scenario: Header and row
- **WHEN** `claude` session `c1` titled `Fix grep` has a hit `fix the app now`
- **THEN** the list shows a header `✻ Fix grep` over the row `fix the app now`

### Requirement: Preview and RET
The selected hit SHALL be previewed by putting point on its session's row in the sessions buffer the prompt was opened from, when that row is shown. `RET` SHALL show the hit's session as `RET` on its row does, loading it when it is not open.

#### Scenario: Moving over hits
- **WHEN** the second hit, in session `x1`, is selected
- **THEN** point in `*Sessions*` is on the `x1` row

#### Scenario: Opening a hit's session
- **WHEN** `RET` is pressed on a hit in session `x1`, which is not open
- **THEN** `x1` is loaded in the current workspace
