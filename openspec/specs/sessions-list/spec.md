# sessions-list Specification

## Purpose
Lists the past agent sessions of the current workspace's project, from every registered agent, in one read-only buffer, so a session can be found again and switched to.

## Requirements

### Requirement: Asking every agent for its sessions
`agent-sessions` (M-x) SHALL send one list request per registered agent, in registry order, each with its own request number, the agent's command and arguments, the current workspace project's primary repo as working directory, and the environment an agent session of that project would start with. When the project's environment is being captured, the requests SHALL wait and SHALL be sent once that capture answers. With no registered agent, it SHALL fail with `No agents registered` and show nothing.

#### Scenario: One request per agent
- **WHEN** two agents are registered and `agent-sessions` runs in a project whose primary repo is `app`
- **THEN** one list request per agent is sent, each with `app` as working directory and its own request number

#### Scenario: The requests wait for the capture
- **WHEN** `agent-sessions` runs while the project's environment is being captured
- **THEN** no list request is sent until the capture answers, and then each is sent with the captured variables first

### Requirement: The sessions buffer
`agent-sessions` SHALL show, in the selected window, a buffer named `*Sessions*<<workspace>>`, made unique, read-only, owned by the current workspace, in `agent-sessions-mode`. A later `agent-sessions` in the same workspace SHALL reuse and rebuild that buffer. It SHALL have no header line: it SHALL start, for each agent in registry order that hasn't answered, with `Listing <agent>...` and for each agent whose answer is an error, with `<agent>: <error>` on one line, then a blank line when any such line was shown and a group follows, then the groups of session rows. When every agent answered and no session was listed, a `No sessions` line SHALL take the groups' place.

#### Scenario: Waiting for answers
- **WHEN** `agent-sessions` runs with agents `claude` and `codex` registered and no answer yet
- **THEN** the buffer reads `Listing claude...` and `Listing codex...`, and is read-only

#### Scenario: An error takes the place of the agent's rows
- **WHEN** `codex` answers with an error `codex can't list sessions` and `claude` with one session
- **THEN** the buffer shows `codex: codex can't list sessions`, a blank line and a group holding the `claude` session only

### Requirement: Session rows
Each row SHALL show four spaces; the first line of the session's title (or its session id when it has no title), at most 60 columns, a longer title cut to 59 and `…`; one space that, when the whole line fits in the window, widens so the line ends at the window's right edge, and otherwise stays one space; the agent's glyph and name, `✻ ` for `claude`, `◆ ` for `codex` and two spaces for any other agent, the name padded to the longest agent name among the listed sessions; two spaces; the session's time right-aligned in 10 columns; and, when any listed session is open in the editor, two spaces and the session's state right-aligned in the width of the longest listed state, blank for a session not open. `updatedAt` SHALL be read as RFC 3339 with `Z` or any numeric offset, with or without a fraction of a second, and taken in the local time zone. The time SHALL read `HH:MM` for a session in Today or Yesterday, the weekday's three-letter English name and `HH:MM` (`Wed 17:30`) in This week, `YYYY-MM-DD` in Older, and be blank when `updatedAt` is missing or unreadable. Within a group, rows SHALL be sorted by `updatedAt`, most recent first; sessions with a missing or unreadable time SHALL come last; sessions with equal times SHALL keep registry order and then the agent's order. A session SHALL be open in the editor while a session buffer exists for a session the same agent started or answered with the same session id, or is loading with that id. Its state SHALL be the first that applies of: `● exited` once its agent process exited or failed to start; `○ loading` before `session/load` answers; `○ starting` before `session/new` answers; `● awaiting permission` while a permission request is pending; `● awaiting answer` while a question that blocks the turn is pending; `● working` while a turn runs; `○ idle` otherwise. Sessions not open in the editor SHALL show no state.

#### Scenario: Two agents in one list, most recent first
- **WHEN** it is 12:00 UTC on Wednesday 2026-10-28, the zone is UTC, `claude` lists sessions updated at 10:00 and 08:00 that day and `codex` one at 09:00
- **THEN** the Today group's rows read `    Fix it ✻ claude       10:00`, `    Port ◆ codex        09:00`, `    Old ✻ claude       08:00`

#### Scenario: Offsets and fractions show in the local zone
- **WHEN** a fixed zone at UTC+02:00 reads `2026-10-28T10:00:00Z`, `2026-10-28T12:30:00.123+02:00` and `2026-10-28T05:15:00.5-04:00`
- **THEN** they are local times 12:00, 12:30 and 11:15 on 2026-10-28

#### Scenario: A missing or unreadable time sorts last and shows blank
- **WHEN** one session has no `updatedAt`, one has `yesterday` and one a valid time two weeks ago
- **THEN** all three are in Older, the valid one first with its date, and the other two after it with blank times

#### Scenario: A long title is cut
- **WHEN** a session's title is 80 characters long
- **THEN** its row shows the first 59 and `…`, and the agent's glyph starts at column 65

#### Scenario: A session open in the editor is marked
- **WHEN** a `claude` session buffer exists for session `s1` between turns and `claude` lists `s1` and `s2`
- **THEN** the `s1` row ends with `○ idle` and the `s2` row shows no state

#### Scenario: The right-hand columns line up at the window's edge
- **WHEN** a 100-column window shows a list with titles of different lengths
- **THEN** every row's time ends at the same column, the window's last

### Requirement: Answers rebuild the list
Each answer to the latest request of an agent SHALL rebuild the buffer's text, keeping the folds. When point was on a session row, it SHALL stay on the row of the same agent's same session if it is listed and shown, go to its group's heading if its group is folded, and otherwise go to the first session row; when point was on a heading, it SHALL stay on that group's heading while the group is shown, else go to the first session row. With no session row shown, point SHALL go to the first heading, or the end of the text when there is none. While no rows are listed, the session or heading point was last on SHALL be remembered for the next rebuild. An answer whose request number isn't the latest of a live sessions buffer SHALL be dropped.

#### Scenario: Point stays on its session
- **WHEN** point is on the `codex` session `c1` row and `claude`'s answer adds a more recent session above it
- **THEN** point is on the `c1` row

#### Scenario: Point stays on its heading
- **WHEN** point is on the Yesterday heading and an answer adds a session to Today
- **THEN** point is on the Yesterday heading

### Requirement: Refreshing the list
`g` in the sessions buffer SHALL send a new list request per registered agent as `agent-sessions` does, show `Listing <agent>...` for each until it answers, drop answers to the earlier requests, and reset the folds to their defaults.

#### Scenario: A late answer is dropped
- **WHEN** `g` is pressed before `claude` answered the first request, and then the first request's answer arrives
- **THEN** the buffer still shows `Listing claude...`, and the second request's answer shows its sessions

### Requirement: Moving, switching and quitting
`n`, `C-n` and `<down>` SHALL move point to the next heading or session row and `p`, `C-p` and `<up>` to the previous one, staying on the last or first at the ends; with none they SHALL fail with `No sessions listed`. Point SHALL land, by a move or a rebuild, on a heading's name (column 2) or a row's title (column 4). Whenever point lands on the first heading or the first session row, by a move or a rebuild, the windows showing it SHALL show the buffer from its first line, so the lines above it are never out of reach. `RET` on the row of a session open in the editor SHALL show that session's buffer in the selected window, switching first to the workspace that owns it if it's another. `RET` on a row not open SHALL load its session with its agent into a new session buffer of the current workspace, shown once loaded; on a `○ loading` row it SHALL echo `Loading <agent> session...` and do nothing else; elsewhere, headings included, it SHALL fail with `Select a session row`. `q` SHALL kill the sessions buffer.

#### Scenario: RET on an open session
- **WHEN** point is on the row of a session open in the editor and `RET` is pressed
- **THEN** the selected window shows that session's buffer

#### Scenario: RET on a session not open
- **WHEN** point is on the `claude` row of session `c2`, which isn't open, and `RET` is pressed
- **THEN** `claude` is spawned to load `c2` in the project's primary repo, the sessions buffer stays shown, the echo area shows `Loading claude session...`, and the `c2` row shows `○ loading` until the load answers, then `○ idle`

#### Scenario: RET again while loading
- **WHEN** `RET` is pressed on a `○ loading` row
- **THEN** the echo area shows `Loading claude session...`, the sessions buffer stays shown and nothing else is spawned

#### Scenario: The header shows again at the first row
- **WHEN** a 5-line window is scrolled down a long list, and then `p` is pressed until point is on the first heading, or `g` is pressed and the answers put point on the first session row
- **THEN** the window's first line is the buffer's first line

#### Scenario: Moving and quitting
- **WHEN** the list has a Today heading over two rows and a Yesterday heading over one, point is on the first row, and `n`, `n`, `n`, `n`, `p` are pressed, then `q`
- **THEN** point visits the second row's title, the Yesterday heading's name, the Yesterday row's title, stays there, and is on the Yesterday heading's name before `q`; after `q` the sessions buffer no longer exists

### Requirement: States follow the sessions
After every message the core handles, each live sessions buffer whose listed sessions' states, folded or shown, no longer match their sessions' states SHALL be rebuilt as an answer rebuilds it, keeping point on its session. A session buffer killed SHALL drop its row's state.

#### Scenario: The state follows a turn and a permission request
- **WHEN** an `○ idle` session is listed, then a prompt is sent, the agent asks for permission, the request is answered, and the turn ends
- **THEN** the row shows `● working`, then `● awaiting permission`, then `● working`, then `○ idle`, with no `g` pressed

#### Scenario: An exited session shows ended
- **WHEN** a listed session's agent process exits
- **THEN** its row shows `● exited`, and once its session buffer is killed, no state

### Requirement: Groups by activity
Listed sessions SHALL be grouped, in this order, as Pinned, Today, Yesterday, This week, Older and Archived, each group shown as a heading line `▾ <name>  <count>` (`▸` when folded), `<count>` being the number of its sessions, followed by its rows unless folded, with one blank line between groups. A group with no session SHALL be left out. By the local date of `updatedAt` against the local date now: the same date or later SHALL be Today; the day before Yesterday; any earlier date from the Monday of the current calendar week (whatever the locale) This week; anything earlier, or a missing or unreadable time, Older. Pinned and Archived SHALL hold no session until pins and archived listings exist. The groups SHALL reflect the date at the latest rebuild.

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

### Requirement: Folding groups
`TAB` (`agent-sessions-toggle`) on a heading SHALL fold its group, hiding its rows and showing `▸`, or unfold it, keeping point on the heading; elsewhere it SHALL do nothing. Every group SHALL start unfolded but Archived, which starts folded. Folds SHALL be kept across answers and state changes, and reset to the defaults by `g` and by a new `agent-sessions`.

#### Scenario: TAB folds and unfolds
- **WHEN** point is on `▾ Today  2` and `TAB` is pressed, then `TAB` again
- **THEN** the buffer first shows `▸ Today  2` with no rows under it and point on the heading, then `▾ Today  2` and its two rows again

#### Scenario: g resets the folds
- **WHEN** Today is folded and `g` is pressed and the agents answer
- **THEN** Today is unfolded

#### Scenario: Point on a folded session goes to its heading
- **WHEN** point is on a Today row, `p` moves it to the Today heading, `TAB` folds Today, and an answer rebuilds the list
- **THEN** point is on the Today heading

### Requirement: Mode line and key hints
The sessions buffer's mode line SHALL read the buffer's name, then the project's name, `<n> sessions` and `<m> open` joined by ` · `, `<n>` counting every listed session and `<m>` those open in the editor, `<m> open` in `status-running` when `<m>` is above 0 and the rest in `mode-line-dim`, and on the right `<position>/<rows>`, `<rows>` being the shown session rows and `<position>` the number of shown session rows up to point's line, at least 1; with no shown session row the right side SHALL be empty. While the sessions buffer is current and neither a message nor a minibuffer prompt shows, the echo line SHALL show the keys bound to move (`agent-sessions-next`/`-previous`), switch (`agent-sessions-visit`), fold (`agent-sessions-toggle`), refresh (`agent-sessions`) and quit (`agent-sessions-quit`), as the workspace list shows its own.

#### Scenario: Counts and position
- **WHEN** project `app` lists three sessions, one open in the editor, and point is on the second shown row
- **THEN** the mode line reads `*Sessions*`, `app · 3 sessions · 1 open` with `1 open` in `status-running`, and `2/3`

#### Scenario: Keys in the echo line
- **WHEN** the sessions buffer is current with no message
- **THEN** the echo line shows `n/p` move, `RET` switch, `TAB` fold, `g` refresh and `q` quit

### Requirement: Faces of the list
The sessions buffer SHALL draw, as frame 14 does: a heading's `▾` or `▸` in `annotation`; its name in `session-list-group`, but Pinned's in `session-list-pinned` and Archived's in `annotation` bold; its count in `session-list-count`; a row's title in the default face; a known agent's glyph and name in its own face, `agent-claude` for `claude` and `agent-codex` for `codex`, and any other agent's name in the default face; a Today row's time in `session-list-today` and any other time in `mode-line-dim`; a state in its own face, `status-running` for `● working`, `status-waiting` for `● awaiting permission` and `● awaiting answer`, `status-failed` for `● exited`, and `mode-line-dim` for `○ idle`, `○ loading` and `○ starting`. The `Listing <agent>...` and `<agent>: <error>` lines, `No sessions` and the spaces between columns SHALL keep the default face. While point is on a heading or session row, that row SHALL be drawn on the `completion-selected` background, as the workspace list draws its selected row. The bundled theme SHALL define the new faces with Gruvbox dark's colours: `session-list-group` `#83a598` bold, `session-list-pinned` `#d3869b` bold, `session-list-count` `#665c54`, `session-list-today` `#8ec07c`, `agent-claude` `#fe8019` and `agent-codex` `#8ec07c`.

#### Scenario: Each piece of a heading and a row has its face
- **WHEN** it is Wednesday 2026-10-28, `claude` lists session `s1` updated that day and open in the editor between turns, and `codex` lists `c1` updated on Monday
- **THEN** in `▾ Today  1` the `▾` is `annotation`, `Today` `session-list-group` and `1` `session-list-count`; on the `s1` row the title has no face, `✻ claude` is `agent-claude`, the time `session-list-today` and `○ idle` `mode-line-dim`; on the `c1` row `◆ codex` is `agent-codex` and `Mon 09:00` `mode-line-dim`

#### Scenario: Live states have their faces
- **WHEN** a listed open session runs a turn, then waits on a permission request, then its agent exits
- **THEN** its state reads `● working` in `status-running`, then `● awaiting permission` in `status-waiting`, then `● exited` in `status-failed`

#### Scenario: Point's row is selected
- **WHEN** point is on a session row
- **THEN** that row, and no other, is drawn on the `completion-selected` background
