## MODIFIED Requirements

### Requirement: Mode line and key hints
The sessions buffer's mode line SHALL read the buffer's name, then the project's name, `<n> sessions` and `<m> open` joined by ` · `, `<n>` counting every listed session and `<m>` those open in the editor, `<m> open` in `status-running` when `<m>` is above 0 and the rest in `mode-line-dim`, and on the right `<position>/<rows>`, `<rows>` being the shown session rows and `<position>` the number of shown session rows up to point's line, at least 1; with no shown session row the right side SHALL be empty. While the sessions buffer is current and neither a message nor a minibuffer prompt shows, the echo line SHALL show the keys bound to move (`agent-sessions-next`/`-previous`), switch (`agent-sessions-visit`), fold (`agent-sessions-toggle`), search (`agent-sessions-search`), pin/unpin (`agent-sessions-pin`), archive (`agent-sessions-archive`), refresh (`agent-sessions`) and quit (`agent-sessions-quit`), as the workspace list shows its own.

#### Scenario: Counts and position
- **WHEN** project `app` lists three sessions, one open in the editor, and point is on the second shown row
- **THEN** the mode line reads `*Sessions*`, `app · 3 sessions · 1 open` with `1 open` in `status-running`, and `2/3`

#### Scenario: Keys in the echo line
- **WHEN** the sessions buffer is current with no message
- **THEN** the echo line shows `n/p` move, `RET` switch, `TAB` fold, `s` search, `P` pin/unpin, `a` archive, `g` refresh and `q` quit
