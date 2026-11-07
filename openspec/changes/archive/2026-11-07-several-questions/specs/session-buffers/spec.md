## MODIFIED Requirements

### Requirement: TAB toggles a tool call
Outside a pasted range and a pending block of several questions, `TAB` in a session buffer (`agent-toggle-tool-call`) with point on a tool call's header line SHALL show its content if folded and hide it otherwise. With point anywhere in an unfolded call's content it SHALL hide the content and move point to the start of the header. The header's fold marker SHALL follow. Shown content SHALL be rendered as markdown, as messages are, with the rule beside each of its lines. A thought row SHALL toggle the same way. On a group line `TAB` SHALL open or fold the group. Inside a group, on a row with nothing to open, on an answered line or on text the fold hides, it SHALL toggle the group, moving point to the group line when it leaves it hidden. With point anywhere else it SHALL echo `No tool call here`.

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

### Requirement: Answering a permission request
`C-c C-y` in a session buffer (`agent-allow-once`) SHALL send the multi-select question, or the block of several questions, whose block point is on, as agent-questions describes; elsewhere it SHALL answer the oldest pending permission request with its first `allow_once` option, passing pending questions by. `C-c C-k` (`agent-reject-once`) SHALL answer the oldest pending request: a permission request with its first `reject_once` option, a question by skipping it, wherever point is. `RET` on an option line of any pending request SHALL answer it with that option; on any other line `RET` sends the prompt as before. The answer SHALL be the `selected` outcome with the option's id, and the request's block SHALL become a single line in `session-thought`, with no rule, that stays in the transcript: `✓ allowed once` for an `allow_once` option, `✓ allowed always` for `allow_always`, `✗ rejected` for `reject_once` and `✗ rejected always` for `reject_always`. With no pending permission request `C-c C-y`, and with no pending request `C-c C-k`, SHALL echo `No permission request is pending`; when the oldest has no option of the kind they SHALL echo `No allow_once option for <title>` (or `reject_once`) and answer nothing.

#### Scenario: Allowing
- **WHEN** a request for `Edit a.txt` is pending and `C-c C-y` is typed in the prompt area
- **THEN** the `Yes` option is selected and the block reads `✓ allowed once`

#### Scenario: Rejecting
- **WHEN** the same request is pending and `C-c C-k` is typed
- **THEN** the `No` option is selected and the block reads `✗ rejected`

#### Scenario: Allowing always
- **WHEN** point is on the line of an `allow_always` option and `RET` is typed
- **THEN** that option is selected and the block reads `✓ allowed always`

#### Scenario: Rejecting always
- **WHEN** point is on the line of a `reject_always` option and `RET` is typed
- **THEN** that option is selected and the block reads `✗ rejected always`

#### Scenario: RET on an option
- **WHEN** point is on the line `2. No (reject_once)` and `RET` is typed
- **THEN** the `No` option is selected and no prompt is sent

#### Scenario: Oldest first
- **WHEN** requests for `A` and then `B` are pending and `C-c C-y` is typed twice
- **THEN** `A` is answered first, then `B`

#### Scenario: Nothing to answer
- **WHEN** no request is pending, or the oldest has no `allow_once` option, and `C-c C-y` is typed
- **THEN** `No permission request is pending`, or `No allow_once option for <title>`, is echoed and nothing is answered

#### Scenario: Allowing passes a question by
- **WHEN** a question and then a permission request for `A` are pending and `C-c C-y` is typed
- **THEN** `A` is answered and the question stays pending

#### Scenario: Sending the multi-select question at point
- **WHEN** a permission request for `A` and then a multi-select question are pending, point is on the question's lines and `C-c C-y` is typed
- **THEN** the question is sent and `A` stays pending
