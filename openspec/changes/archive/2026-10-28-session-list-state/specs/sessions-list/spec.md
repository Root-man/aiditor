## MODIFIED Requirements

### Requirement: Session rows
Each row SHALL show the agent name padded to the longest agent name among the rows, two spaces, the session's `updatedAt` in the local time zone as `YYYY-MM-DD HH:MM` (blank, the same width, when missing or not RFC 3339), two spaces, the session's state followed by a space when the session is open in the editor, and the first line of the session's title, or its session id when it has no title. `updatedAt` SHALL be read as RFC 3339 with `Z` or any numeric offset, with or without a fraction of a second. Rows from every agent SHALL be sorted by `updatedAt`, most recent first; sessions with a missing or unreadable time SHALL come last; sessions with equal times SHALL keep registry order and then the agent's order. A session SHALL be open in the editor while a session buffer exists for a session the same agent started or answered with the same session id. Its state SHALL be the first that applies of: `[ended]` once its agent process exited or failed to start; `[starting]` before `session/new` answers; `[waiting]` while a permission request is pending; `[working]` while a turn runs; `[idle]` otherwise. Sessions not open in the editor SHALL show no state.

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
- **WHEN** a `claude` session buffer exists for session `s1` between turns and `claude` lists `s1` and `s2`
- **THEN** the `s1` row shows `[idle]` before its title and the `s2` row shows no state

### Requirement: Moving, switching and quitting
`n`, `C-n` and `<down>` SHALL move point to the start of the next session row and `p`, `C-p` and `<up>` to the previous one, staying on the last or first row at the ends; with no rows they SHALL fail with `No sessions listed`. Whenever point lands on the first row, by a move or a rebuild, the windows showing it SHALL show the buffer from its first line, so the header is never out of reach. `RET` on the row of a session open in the editor SHALL show that session's buffer in the selected window, switching first to the workspace that owns it if it's another. `RET` on a row not open SHALL fail with `Loading sessions isn't supported yet`, and elsewhere with `Select a session row`. `q` SHALL kill the sessions buffer.

#### Scenario: RET on an open session
- **WHEN** point is on the row of a session open in the editor and `RET` is pressed
- **THEN** the selected window shows that session's buffer

#### Scenario: RET on a session not open
- **WHEN** point is on a row without a state and `RET` is pressed
- **THEN** the echo area shows `Loading sessions isn't supported yet` and the sessions buffer stays shown

#### Scenario: The header shows again at the first row
- **WHEN** a 5-line window is scrolled down a long list, and then `p` is pressed until point is on the first row, or `g` is pressed and the answers put point on the first row
- **THEN** the window's first line is the header

#### Scenario: Moving and quitting
- **WHEN** the list has three rows, point is on the first, and `n`, `n`, `n`, `p` are pressed, then `q`
- **THEN** point is on the second row before `q`, and after `q` the sessions buffer no longer exists

## ADDED Requirements

### Requirement: States follow the sessions
After every message the core handles, each live sessions buffer whose rows' states no longer match their sessions' states SHALL be rebuilt as an answer rebuilds it, keeping point on its session. A session buffer killed SHALL drop its row's state.

#### Scenario: The state follows a turn and a permission request
- **WHEN** an `[idle]` session is listed, then a prompt is sent, the agent asks for permission, the request is answered, and the turn ends
- **THEN** the row shows `[working]`, then `[waiting]`, then `[working]`, then `[idle]`, with no `g` pressed

#### Scenario: An exited session shows ended
- **WHEN** a listed session's agent process exits
- **THEN** its row shows `[ended]`, and once its session buffer is killed, no state
