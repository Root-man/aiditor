## MODIFIED Requirements

### Requirement: TAB toggles a tool call
Outside a pasted range and a pending block of several questions, `TAB` in a session buffer (`agent-toggle-tool-call`) with point on a tool call's header line SHALL show its content if folded and hide it otherwise. With point anywhere in an unfolded call's content it SHALL hide the content and move point to the start of the header. The header's fold marker SHALL follow. Shown content SHALL be rendered as markdown, as messages are, with the rule beside each of its lines. A thought row, and an answered question's line that sent something, SHALL toggle the same way. On a group line `TAB` SHALL open or fold the group. Inside a group, on a row with nothing to open, on another answered line or on text the fold hides, it SHALL toggle the group, moving point to the group line when it leaves it hidden. With point anywhere else it SHALL echo `No tool call here`.

#### Scenario: Unfolding and folding
- **WHEN** point is on a folded call's header and `TAB` is typed, then `TAB` again
- **THEN** the content shows under the header with the rule beside it and its fences hidden, then hides again, the fold marker going from `▸` to `▾` and back

#### Scenario: Folding from inside the content
- **WHEN** a call is unfolded, point is on a line of its content and `TAB` is typed
- **THEN** the content hides and point is at the start of the header

#### Scenario: Not on a header
- **WHEN** `TAB` is typed in the prompt area outside any pasted range
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

#### Scenario: TAB in a block of several questions
- **WHEN** point is on the lines of a pending block of several questions inside a tool group and `TAB` is typed
- **THEN** the block opens its next question and the group stays as it was
