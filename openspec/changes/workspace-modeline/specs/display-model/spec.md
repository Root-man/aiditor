## MODIFIED Requirements

### Requirement: The frame display names the current project and workspace
The frame display SHALL carry workspace modeline spans for every workspace in creation order, each as `project › workspace`, separated by ` │ ` in `separator`. The current entry SHALL use the bright `workspace-bar` foreground with its workspace name in `workspace-current`; the other entries SHALL use `mode-line-dim`. Project names SHALL be bold and ` › ` SHALL use `separator`. Editing, splitting windows and prompting SHALL retain the workspace overview.

#### Scenario: Names follow the current workspace
- **GIVEN** workspaces `main` of `home` and then `w2` of `other`
- **WHEN** `w2` is renamed `w3` and `main` becomes current
- **THEN** the bar reads `home › main │ other › w3`, with `main` highlighted and the other entry dim

#### Scenario: Names stay while editing and prompting
- **GIVEN** a workspace overview
- **WHEN** the window is split, another buffer shown or the minibuffer opened
- **THEN** the workspace overview keeps its order and current highlight

### Requirement: The workspace modeline names are truncated to the frame width
The workspace modeline SHALL reserve five columns for the clock and two columns of separation. If all entries do not fit the remaining width at the workspace font size, it SHALL retain a contiguous range containing the current workspace and as many immediate neighbours as fit, expanding toward the nearest neighbours and preferring the preceding one on ties. Hidden entries SHALL fold into a trailing `+N` entry, separated by ` │ `, with the most urgent hidden activity dot. If the current entry itself is too wide, its names SHALL share the available room after separators and its dot: a name fitting half stays whole and the other takes the rest, otherwise each takes half with the workspace receiving the odd column. A cut name SHALL end in `…`. At widths insufficient for everything, retain the current names before the overflow entry and omit parts that cannot fit; the line SHALL stay within its budget.

#### Scenario: Overflow around a middle workspace
- **GIVEN** five short workspace entries and room for three entries plus overflow
- **WHEN** the middle workspace becomes current
- **THEN** the bar shows it and its immediate neighbours in creation order, then `+2` with the most urgent hidden dot

#### Scenario: A long current name
- **GIVEN** a current workspace name too long for the modeline
- **WHEN** the frame narrows
- **THEN** the current entry remains visible, its names shortened with `…`, and the bar fits the remaining width

#### Scenario: A wider workspace font
- **GIVEN** a frame with overflowing workspaces
- **WHEN** the workspace font size increases
- **THEN** fewer entries fit and the current entry stays visible

#### Scenario: Names that fit stay whole
- **GIVEN** project `aiditor` and workspace `watcher-fix`
- **WHEN** the entry has 40 columns after reserving the clock
- **THEN** both names stay whole

#### Scenario: A long workspace name is cut
- **GIVEN** project `Legacy crap` and a 60-character workspace name
- **WHEN** the current entry has 40 columns after reserving the clock
- **THEN** the project stays whole and the workspace is cut to 26 characters ending in `…`

#### Scenario: Two long names are both cut
- **GIVEN** two 30-character project and workspace names
- **WHEN** the current entry has 23 columns after reserving the clock
- **THEN** each name is cut to 10 characters ending in `…`

## ADDED Requirements

### Requirement: Workspace dots aggregate agent session activity
Each workspace SHALL show a `●` after its name when any owned session is waiting for permission, failed to start or exited, or working on a prompt. Priority across sessions SHALL be waiting in `status-waiting`, then failed/exited in `status-failed`, then working in `status-running`. Live idle and starting sessions SHALL have no dot. The dot SHALL follow activity even in other workspaces; killing a session or workspace SHALL remove its contribution. Hidden dots SHALL aggregate with the same priority.

#### Scenario: Mixed activity across sessions
- **GIVEN** a workspace with working, failed and permission-waiting sessions
- **WHEN** the bar is built
- **THEN** its dot uses `status-waiting`
- **WHEN** the waiting session becomes idle
- **THEN** its dot uses `status-failed`
- **WHEN** the failed session is killed
- **THEN** its dot uses `status-running`

#### Scenario: Idle after completing a prompt
- **GIVEN** a working session in another workspace
- **WHEN** it completes its prompt with no pending permission
- **THEN** that workspace returns to having no dot
