# Spec Delta

## ADDED Requirements

### Requirement: Consecutive tool calls fold into a group
A run of tool calls SHALL show as one group once it holds two calls. A run SHALL end at an agent message, a user prompt, a line of the session's own or the first show of the plan; an update to a plan already shown SHALL NOT end it. Thoughts and permission blocks SHALL NOT end a run: those after its first call, and thoughts before it, SHALL be rows of the group. A run with one call SHALL show as it would alone, with no group line, its thoughts as ordinary blocks. A group SHALL start folded, showing only its group line, unless its first call was already open, in which case it starts open. It SHALL stay folded or open as it grows and as its calls update, and each call SHALL stay folded or open.

Rows SHALL sit on consecutive lines, without blank lines between them, and each tool call or thought row SHALL be indented two columns. A blank line SHALL follow the group's last row.

#### Scenario: Two calls fold
- **WHEN** two tool calls arrive in a row
- **THEN** one group line shows and neither call's row does

#### Scenario: One call has no group line
- **WHEN** one tool call arrives and an agent message follows
- **THEN** the call shows as its header with no group line

#### Scenario: A message starts a new group
- **WHEN** two calls, an agent message and two more calls arrive
- **THEN** two group lines show with the message between them

#### Scenario: A plan update doesn't split a group
- **GIVEN** a plan is shown
- **WHEN** a call, a plan update and a second call arrive
- **THEN** one group holds both calls

#### Scenario: A thought goes inside
- **WHEN** a thought, a call, a thought and a call arrive
- **THEN** one group holds all four as rows, in order

#### Scenario: A growing group stays folded
- **WHEN** a third call and updates to the first two arrive at a folded group
- **THEN** only the group line shows

#### Scenario: An open call stays open as its run becomes a group
- **WHEN** the single call of a run is opened and a second call arrives
- **THEN** the group is open with the first call's content showing

### Requirement: The group line
The group line SHALL read `<fold> <icon> <verbs>` in `session-thought`, `▸` when folded and `▾` when open, with no rule beside it. The verbs SHALL be one phrase per class of kind in order of first appearance, comma separated, only the first capitalized: `read files`, `searched code`, `edited files` (edit, delete and move), `ran commands`, `fetched pages`, `used tools` (anything else). A class with an unfinished call SHALL read in the present tense: `reading files`, `searching code`, `editing files`, `running commands`, `fetching pages`, `using tools`. The icon SHALL be the first call's. Right-aligned, it SHALL show `<n> failed` in `status-failed` when calls failed, and, once every call has finished, the time from the first call's arrival to the last one's finish in `annotation`, two spaces after any failure count. A replayed call SHALL leave the duration out.

#### Scenario: Verbs follow first appearance
- **WHEN** a group's finished calls are a read, an execute and a read
- **THEN** its line reads `Read files, ran commands`

#### Scenario: The running verb is present
- **WHEN** a group holds a finished edit and an unfinished execute
- **THEN** its line reads `Edited files, running commands` and has no duration

#### Scenario: Failures show when folded
- **WHEN** one call of a folded group fails and the others finish
- **THEN** its line ends with `1 failed` in `status-failed` then the duration

#### Scenario: Duration spans the group
- **WHEN** a call arrives, a second arrives 1s later and it finishes 2s after that, the first having finished
- **THEN** the group line shows `3.0s`

### Requirement: Thought rows
A thought in a group SHALL show as one row, `  ▸ <first line>` in `session-thought`, where the first line is its `Thought for <n>s` line, or its text's first line when replayed; the rest SHALL be folded under it. While it streams it SHALL show as a thought block does, in an open group.

#### Scenario: A thought row folds its text
- **WHEN** a group holding a thought `hmm` is opened
- **THEN** the row reads `  ▸ Thought for 0s` and `hmm` shows only after `TAB` on the row

### Requirement: Waiting calls show under a folded group
While a permission request is pending for a call of a folded group, that call's row SHALL show under the group line. A pending permission block in a group SHALL show too. Once answered, the answered line SHALL stay in the group as a row and hide with the rest.

#### Scenario: Waiting under the fold
- **WHEN** a folded group's second call gets a permission request
- **THEN** the group line, the call's row and the request's block show, and not the first call

#### Scenario: Answering folds back
- **WHEN** that request is answered
- **THEN** only the group line shows, and opening the group shows the answered line under the call

### Requirement: Motion across folded groups
`C-n` and `C-p` SHALL pass a folded group as one line, as they pass a folded call. Search SHALL still match its hidden text and kill and copy SHALL still yield it. `RET` SHALL show a call's output only from a row that shows.

#### Scenario: Line motion
- **WHEN** point is on the line above a folded group and `C-n` is typed twice
- **THEN** point is on the group line, then on the line after the group

#### Scenario: Search reaches hidden rows
- **WHEN** `C-s` searches for a hidden call's target
- **THEN** it matches

#### Scenario: A killed region holds hidden rows
- **WHEN** a region spanning a folded group is copied
- **THEN** the copy holds every row's text

### Requirement: Replayed history groups
Calls replayed by `session/load` SHALL group as live ones do, without durations.

#### Scenario: A loaded session
- **WHEN** a load replays two finished calls
- **THEN** one group line shows with no duration

## MODIFIED Requirements

### Requirement: TAB toggles a tool call
`TAB` in a session buffer (`agent-toggle-tool-call`) with point on a tool call's header line SHALL show its content if folded and hide it otherwise. With point anywhere in an unfolded call's content it SHALL hide the content and move point to the start of the header. The header's fold marker SHALL follow. Shown content SHALL be rendered as markdown, as messages are, with the rule beside each of its lines. A thought row SHALL toggle the same way. On a group line `TAB` SHALL open or fold the group. Inside a group, on a row with nothing to open, on an answered line or on text the fold hides, it SHALL toggle the group, moving point to the group line when it leaves it hidden. With point anywhere else it SHALL echo `No tool call here`.

#### Scenario: Unfolding and folding
- **WHEN** point is on a folded call's header and `TAB` is typed, then `TAB` again
- **THEN** the content shows under the header with the rule beside it and its fences hidden, then hides again, the fold marker going from `▸` to `▾` and back

#### Scenario: Folding from inside the content
- **WHEN** a call is unfolded, point is on a line of its content and `TAB` is typed
- **THEN** the content hides and point is at the start of the header

#### Scenario: Not on a header
- **WHEN** `TAB` is typed in the prompt area
- **THEN** `No tool call here` is echoed and nothing changes

#### Scenario: A call without content
- **WHEN** `TAB` is typed on the header of a call without content and an update then brings output
- **THEN** the output arrives folded

#### Scenario: Unfolded content stays open
- **WHEN** a call is unfolded and an update brings new output
- **THEN** the call stays unfolded, showing the new output

#### Scenario: Opening a group, then a row
- **WHEN** `TAB` is typed on a folded group line, then on a row
- **THEN** the rows show, then that call's content shows, and the group line's marker reads `▾`

#### Scenario: Folding back level by level
- **WHEN** point is in an open row's content in an open group and `TAB` is typed, then `TAB` on the group line
- **THEN** the call folds with point on its row, then the group folds

#### Scenario: A row with nothing to open
- **WHEN** `TAB` is typed on a row without content in an open group
- **THEN** the group folds and point is on its line
