# Spec Delta

## Purpose

Lists the past agent sessions of the current workspace's project, from every registered agent, in one read-only buffer, so a session can be found again and switched to.

## ADDED Requirements

### Requirement: Asking every agent for its sessions
`agent-sessions` (M-x) SHALL send one list request per registered agent, in registry order, each with its own request number, the agent's command and arguments, the current workspace project's primary repo as working directory, and the environment an agent session of that project would start with. When the project's environment is being captured, the requests SHALL wait and SHALL be sent once that capture answers. With no registered agent, it SHALL fail with `No agents registered` and show nothing.

#### Scenario: One request per agent
- **WHEN** two agents are registered and `agent-sessions` runs in a project whose primary repo is `app`
- **THEN** one list request per agent is sent, each with `app` as working directory and its own request number

#### Scenario: The requests wait for the capture
- **WHEN** `agent-sessions` runs while the project's environment is being captured
- **THEN** no list request is sent until the capture answers, and then each is sent with the captured variables first

### Requirement: The sessions buffer
`agent-sessions` SHALL show, in the selected window, a buffer named `*Sessions*<<workspace>>`, made unique, read-only, owned by the current workspace, in `agent-sessions-mode`. A later `agent-sessions` in the same workspace SHALL reuse and rebuild that buffer. Its first line SHALL be `Sessions — RET switch, g refresh, n/p move, q quit`, then a blank line, then for each agent in registry order that hasn't answered `Listing <agent>...` and for each agent whose answer is an error `<agent>: <error>`, then one row per listed session. When every agent answered and no session was listed, a `No sessions` line SHALL take the rows' place.

#### Scenario: Waiting for answers
- **WHEN** `agent-sessions` runs with agents `claude` and `codex` registered and no answer yet
- **THEN** the buffer shows the header, a blank line, `Listing claude...` and `Listing codex...`, and is read-only

#### Scenario: An error takes the place of the agent's rows
- **WHEN** `codex` answers with an error `codex can't list sessions` and `claude` with one session
- **THEN** the buffer shows `codex: codex can't list sessions` and a row for the `claude` session only

### Requirement: Session rows
Each row SHALL show the agent name padded to the longest agent name among the rows, two spaces, the session's `updatedAt` in the local time zone as `YYYY-MM-DD HH:MM` (blank, the same width, when missing or not RFC 3339), two spaces, `[open] ` when the session is open in the editor, and the first line of the session's title, or its session id when it has no title. `updatedAt` SHALL be read as RFC 3339 with `Z` or any numeric offset, with or without a fraction of a second. Rows from every agent SHALL be sorted by `updatedAt`, most recent first; sessions with a missing or unreadable time SHALL come last; sessions with equal times SHALL keep registry order and then the agent's order. A session SHALL be open in the editor while a session buffer exists for a session the same agent started or answered with the same session id.

#### Scenario: Two agents in one list, most recent first
- **WHEN** `claude` lists sessions updated at 10:00 and 08:00 UTC and `codex` one at 09:00 UTC, all on one day, in a zone at UTC
- **THEN** the rows read claude 10:00, codex 09:00, claude 08:00

#### Scenario: Offsets and fractions show in the local zone
- **WHEN** a fixed zone at UTC+02:00 formats `2026-10-28T10:00:00Z`, `2026-10-28T12:30:00.123+02:00` and `2026-10-28T05:15:00.5-04:00`
- **THEN** they show as `2026-10-28 12:00`, `2026-10-28 12:30` and `2026-10-28 11:15`

#### Scenario: A missing or unreadable time sorts last and shows blank
- **WHEN** one session has no `updatedAt`, one has `yesterday` and one a valid time
- **THEN** the valid one is first, and the other two show blank times after it

#### Scenario: A session open in the editor is marked
- **WHEN** a `claude` session buffer exists for session `s1` and `claude` lists `s1` and `s2`
- **THEN** the `s1` row shows `[open]` before its title and the `s2` row doesn't

### Requirement: Answers rebuild the list
Each answer to the latest request of an agent SHALL rebuild the buffer's text. When point was on a session row, it SHALL stay on the row of the same agent's same session if it is still listed, else go to the first row; otherwise it SHALL go to the first row, or the end of the text when there are none. An answer whose request number isn't the latest of a live sessions buffer SHALL be dropped.

#### Scenario: Point stays on its session
- **WHEN** point is on the `codex` session `c1` row and `claude`'s answer adds a more recent session above it
- **THEN** point is on the `c1` row

### Requirement: Refreshing the list
`g` in the sessions buffer SHALL send a new list request per registered agent as `agent-sessions` does, show `Listing <agent>...` for each until it answers, and drop answers to the earlier requests.

#### Scenario: A late answer is dropped
- **WHEN** `g` is pressed before `claude` answered the first request, and then the first request's answer arrives
- **THEN** the buffer still shows `Listing claude...`, and the second request's answer shows its sessions

### Requirement: Moving, switching and quitting
`n`, `C-n` and `<down>` SHALL move point to the start of the next session row and `p`, `C-p` and `<up>` to the previous one, staying on the last or first row at the ends; with no rows they SHALL fail with `No sessions listed`. `RET` on an `[open]` row SHALL show that session's buffer in the selected window, switching first to the workspace that owns it if it's another. `RET` on a row not open SHALL fail with `Loading sessions isn't supported yet`, and elsewhere with `Select a session row`. `q` SHALL kill the sessions buffer.

#### Scenario: RET on an open session
- **WHEN** point is on an `[open]` row and `RET` is pressed
- **THEN** the selected window shows that session's buffer

#### Scenario: RET on a session not open
- **WHEN** point is on a row without `[open]` and `RET` is pressed
- **THEN** the echo area shows `Loading sessions isn't supported yet` and the sessions buffer stays shown

#### Scenario: Moving and quitting
- **WHEN** the list has three rows, point is on the first, and `n`, `n`, `n`, `p` are pressed, then `q`
- **THEN** point is on the second row before `q`, and after `q` the sessions buffer no longer exists
