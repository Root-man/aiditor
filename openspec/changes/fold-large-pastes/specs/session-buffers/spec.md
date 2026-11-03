## ADDED Requirements

### Requirement: Long prompt yanks fold
A yank into the session prompt strictly longer than `paste_fold_lines` SHALL appear as ` ≡ Pasted N lines ` in the boxed attachment face. The complete original text SHALL remain in the buffer. Yanks at or below the limit and yanks into ordinary buffers SHALL remain plain. A trailing newline SHALL not count as an extra line. Yank-pop SHALL apply the same threshold to the replacement yank.

#### Scenario: Long yank
- **GIVEN** the default threshold and a 21-line clipboard
- **WHEN** it is yanked into the session prompt
- **THEN** the prompt displays `≡ Pasted 21 lines` and its buffer contains the complete pasted text

#### Scenario: At the threshold
- **GIVEN** the default threshold and a 20-line clipboard
- **WHEN** it is yanked into the session prompt
- **THEN** all 20 lines remain plain text

### Requirement: TAB previews a paste
TAB on a folded paste SHALL retain its chip header and show the first four original lines below it in the session-block face, followed by `… N more lines` in the annotation face when additional lines remain. For fewer than four lines it SHALL show all available lines. TAB in the paste SHALL collapse it again and place point at its start. This behavior SHALL take precedence over tool folding only within a paste.

#### Scenario: Preview and collapse
- **GIVEN** a folded 21-line paste
- **WHEN** TAB is pressed at its start and then again inside it
- **THEN** the chip shows four preview lines and `… 17 more lines`, then folds again with point at its start

### Requirement: Folded pastes send complete input
Sending a prompt SHALL transmit all its pasted text in order with surrounding text and other chips. The sent transcript SHALL show each paste folded, including one whose preview was open before sending. New prompt text and streamed output SHALL not alter the pasted input or lose its fold.

#### Scenario: Send an open preview
- **GIVEN** a long paste surrounded by ordinary prompt text with its preview open
- **WHEN** the prompt is sent
- **THEN** the request contains all original pasted and surrounding text and the transcript shows a folded paste above the new prompt

#### Scenario: Output arrives while composing
- **GIVEN** a folded paste in the prompt
- **WHEN** streamed agent output is inserted above it and surrounding prompt text is edited
- **THEN** its fold follows its text and sending still includes every pasted line

## MODIFIED Requirements

### Requirement: TAB toggles a tool call
Outside a pasted range, `TAB` in a session buffer (`agent-toggle-tool-call`) with point on a tool call's header line SHALL show its content if folded and hide it otherwise. With point anywhere in an unfolded call's content it SHALL hide the content and move point to the start of the header. The header's fold marker SHALL follow. Shown content SHALL be rendered as markdown, as messages are, with the rule beside each of its lines. A thought row SHALL toggle the same way. On a group line `TAB` SHALL open or fold the group. Inside a group, on a row with nothing to open, on an answered line or on text the fold hides, it SHALL toggle the group, moving point to the group line when it leaves it hidden. With point anywhere else it SHALL echo `No tool call here`.

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
