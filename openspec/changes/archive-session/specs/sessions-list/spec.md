## ADDED Requirements

### Requirement: Archiving sessions
`a` (`agent-sessions-archive`) on a row of an agent registered as `codex` SHALL send that agent a new list request that deletes the session first, drop the row from every live sessions buffer at once, and take only that request's answer for the agent. Other agents SHALL fail with `Not supported yet`; elsewhere it SHALL fail with `Select a session row`. An open session's agent SHALL be named to stop first; its buffer SHALL stay.

#### Scenario: Archiving a Codex session
- **WHEN** point is on the `codex` session `x1` row and `a` is pressed
- **THEN** a list request for `codex` that deletes `x1` is sent, the `x1` row is gone from the buffer, and the answer to it rebuilds the `codex` rows

#### Scenario: Archiving a Claude session
- **WHEN** point is on a `claude` session row and `a` is pressed
- **THEN** it fails with `Not supported yet` and no request is sent

#### Scenario: Archiving an open session
- **WHEN** the `codex` session `x1` is open in the editor and `a` is pressed on its row
- **THEN** the list request names its agent to stop, and its session buffer is still live

## MODIFIED Requirements

### Requirement: Mode line and key hints
The sessions buffer's mode line SHALL read the buffer's name, then the project's name, `<n> sessions` and `<m> open` joined by ` · `, `<n>` counting every listed session and `<m>` those open in the editor, `<m> open` in `status-running` when `<m>` is above 0 and the rest in `mode-line-dim`, and on the right `<position>/<rows>`, `<rows>` being the shown session rows and `<position>` the number of shown session rows up to point's line, at least 1; with no shown session row the right side SHALL be empty. While the sessions buffer is current and neither a message nor a minibuffer prompt shows, the echo line SHALL show the keys bound to move (`agent-sessions-next`/`-previous`), switch (`agent-sessions-visit`), fold (`agent-sessions-toggle`), pin/unpin (`agent-sessions-pin`), archive (`agent-sessions-archive`), refresh (`agent-sessions`) and quit (`agent-sessions-quit`), as the workspace list shows its own.

#### Scenario: Counts and position
- **WHEN** project `app` lists three sessions, one open in the editor, and point is on the second shown row
- **THEN** the mode line reads `*Sessions*`, `app · 3 sessions · 1 open` with `1 open` in `status-running`, and `2/3`

#### Scenario: Keys in the echo line
- **WHEN** the sessions buffer is current with no message
- **THEN** the echo line shows `n/p` move, `RET` switch, `TAB` fold, `P` pin/unpin, `a` archive, `g` refresh and `q` quit
