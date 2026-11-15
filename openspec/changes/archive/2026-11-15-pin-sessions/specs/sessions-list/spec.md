## ADDED Requirements

### Requirement: Pinning sessions
`P` (`agent-sessions-pin`) on a session row SHALL pin the session when it isn't pinned and unpin it when it is, rebuild every live sessions buffer of the same project, and keep point on the session's row; elsewhere, headings included, it SHALL fail with `Select a session row`. A pin SHALL be one agent's session id in one project, known by its primary root.

#### Scenario: The same id from another agent isn't pinned
- **WHEN** `claude` and `codex` both list a session `s1` and `P` pins `claude`'s
- **THEN** only `claude`'s `s1` lists under Pinned

#### Scenario: P pins and unpins
- **WHEN** point is on the `codex` session `x1` row in Today and `P` is pressed, then `P` again
- **THEN** the buffer first shows `▾ Pinned  1` with the `x1` row, a blank line and Today without it, point on the `x1` row; then Today with `x1` again and no Pinned heading

#### Scenario: P on a heading
- **WHEN** point is on a heading and `P` is pressed
- **THEN** it fails with `Select a session row`

### Requirement: Pins survive a restart
Pins SHALL be kept in `sessions.toml` in the platform's local data directory, written in full, atomically, on each change, and read at launch, so a session pinned before a restart lists under Pinned after it. The latest change SHALL be what the file holds once writes finish. A file that can't be read or parsed SHALL be reported in `*Messages*`, and the editor SHALL start with no pins; a missing file SHALL mean no pins.

#### Scenario: A pin is kept
- **WHEN** a session is pinned, the editor restarts with the file as written, and its agent lists it again
- **THEN** it lists under Pinned

#### Scenario: An unreadable file
- **WHEN** `sessions.toml` doesn't parse at launch
- **THEN** `*Messages*` names the file and the error, and nothing is pinned

### Requirement: Pins of gone sessions are dropped
When an agent's list for a project answers without error and without a session that agent has pinned in that project, the pin SHALL be removed and the file rewritten, with no message. Pins of other agents and other projects, and pins of an agent whose answer is an error, SHALL be kept.

#### Scenario: A gone session's pin is dropped
- **WHEN** `claude`'s session `c9` is pinned in the project and `claude`'s list answers without `c9`
- **THEN** the file no longer holds the `c9` pin and no message is shown

#### Scenario: Another project's pin stays
- **WHEN** a session is pinned in project `lib` and the list for project `app` answers
- **THEN** its pin stays in the file

## MODIFIED Requirements

### Requirement: Groups by activity
Listed sessions SHALL be grouped, in this order, as Pinned, Today, Yesterday, This week, Older and Archived, each group shown as a heading line `▾ <name>  <count>` (`▸` when folded), `<count>` being the number of its sessions, followed by its rows unless folded, with one blank line between groups. A group with no session SHALL be left out. Pinned SHALL hold the pinned sessions, most recent first, each row's time shown as its activity group would show it. Other sessions SHALL be grouped by the local date of `updatedAt` against the local date now: the same date or later SHALL be Today; the day before Yesterday; any earlier date from the Monday of the current calendar week (whatever the locale) This week; anything earlier, or a missing or unreadable time, Older. Archived SHALL hold no session until archived listings exist. The groups SHALL reflect the date at the latest rebuild.

#### Scenario: Day boundaries
- **WHEN** it is 00:30 on Wednesday 2026-10-28 and sessions were updated at 00:10 that day, 23:50 on Tuesday 2026-10-27 and 23:50 on Monday 2026-10-26
- **THEN** they fall in Today (`00:10`), Yesterday (`23:50`) and This week (`Mon 23:50`)

#### Scenario: The week starts on Monday
- **WHEN** it is Wednesday 2026-10-28 and sessions were updated on Monday 2026-10-26 at 00:00 and on Sunday 2026-10-25 at 23:59
- **THEN** the first is in This week and the second in Older as `2026-10-25`

#### Scenario: On a Monday, two days back is older
- **WHEN** it is Monday 2026-10-26 and sessions were updated on Sunday 2026-10-25 and Saturday 2026-10-24
- **THEN** Sunday's is in Yesterday and Saturday's in Older, and no This week heading shows

#### Scenario: Headings count their sessions
- **WHEN** three sessions are in Today and one in Older
- **THEN** the buffer shows `▾ Today  3`, three rows, a blank line, `▾ Older  1` and one row

#### Scenario: A pinned row keeps its time
- **WHEN** a session updated two weeks ago on 2026-10-14 is pinned
- **THEN** it lists under Pinned with the time `2026-10-14` in `mode-line-dim`

### Requirement: Mode line and key hints
The sessions buffer's mode line SHALL read the buffer's name, then the project's name, `<n> sessions` and `<m> open` joined by ` · `, `<n>` counting every listed session and `<m>` those open in the editor, `<m> open` in `status-running` when `<m>` is above 0 and the rest in `mode-line-dim`, and on the right `<position>/<rows>`, `<rows>` being the shown session rows and `<position>` the number of shown session rows up to point's line, at least 1; with no shown session row the right side SHALL be empty. While the sessions buffer is current and neither a message nor a minibuffer prompt shows, the echo line SHALL show the keys bound to move (`agent-sessions-next`/`-previous`), switch (`agent-sessions-visit`), fold (`agent-sessions-toggle`), pin/unpin (`agent-sessions-pin`), refresh (`agent-sessions`) and quit (`agent-sessions-quit`), as the workspace list shows its own.

#### Scenario: Counts and position
- **WHEN** project `app` lists three sessions, one open in the editor, and point is on the second shown row
- **THEN** the mode line reads `*Sessions*`, `app · 3 sessions · 1 open` with `1 open` in `status-running`, and `2/3`

#### Scenario: Keys in the echo line
- **WHEN** the sessions buffer is current with no message
- **THEN** the echo line shows `n/p` move, `RET` switch, `TAB` fold, `P` pin/unpin, `g` refresh and `q` quit
