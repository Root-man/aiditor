## ADDED Requirements

### Requirement: Answered questions
A closed question's block SHALL become one line with no rule, `▸ <mark> <label>  <outcome>`, followed by the clock time at the right edge in `annotation` when the clock is known. The label SHALL be the question's message, or `<n> questions` for a block of several fields. The mark and outcome SHALL be `✓` and the answer when it was sent, `–` and `Skipped`, `✗` and `Cancelled`, or `○` and `Closed` (`Closed, the turn ended first` when the editor closed it at a turn's end). `▸` SHALL be in `session-thought`, `✓` in `status-running`, `✗` in `status-failed`, `–` and `○` in `session-thought`, the label in the default face and the outcome in `session-thought`. In a tool group the line SHALL sit two columns in, as a row, and fold away with the group. With point on a sent question's line, `TAB` SHALL open it: `▾` in place of `▸` and, under it, a line per field that was sent, ` <name>  <answer>` with names padded to the longest and in `session-thought`, on the `session-block` background from the label's column; a field is named by its title, or its key without one. `TAB` on the line or one of those lines SHALL fold them again, moving point to the line's start from below it. A question that sent nothing has nothing to open. When the question named a shown tool call, that call SHALL show no row of its own once the question is closed: the line stands for it, where the block was.

#### Scenario: Each outcome shrinks to its line
- **WHEN** questions asking `Which?` are answered `A`, skipped, cancelled and withdrawn
- **THEN** their blocks read `▸ ✓ Which?  A`, `▸ – Which?  Skipped`, `▸ ✗ Which?  Cancelled` and `▸ ○ Which?  Closed`

#### Scenario: TAB opens what was sent
- **WHEN** a block of fields `Status` and `Commit` is sent with `Both` and `One`, and `TAB` is typed on its line, then again
- **THEN** the line reads `▸ ✓ 2 questions  Both · One`, then `▾ ✓ 2 questions  Both · One` above ` Status  Both` and ` Commit  One` on `session-block`, then folds back to the line

#### Scenario: Nothing was sent
- **WHEN** `TAB` is typed on a skipped question's line outside a group
- **THEN** `No tool call here` is echoed and the line stays

#### Scenario: The line takes the tool call's place
- **WHEN** a question names tool call `t`, shown alone above it, and is answered
- **THEN** `t`'s row no longer shows and the answered line does, where the block was, also after `t` completes

#### Scenario: A session/load replay
- **WHEN** a loaded session's history replays a question's tool call
- **THEN** it shows as a tool call row, no question block shows and the status doesn't read `● awaiting answer`

## MODIFIED Requirements

### Requirement: Answering a question
On a pending question's lines, a digit `1` to `9` SHALL answer it with that option at once, and `RET` on an option line SHALL answer it with that option; the answer SHALL be `accept` with the choice field set to the option's value. `o` on a question's lines SHALL read `Own answer: ` in the minibuffer when the question has a companion, and answer `accept` in the adapter's shape: the companion alone for a `_custom` companion, the choice field set to `None of the above` and the companion to the text for a `user_note` companion. An own answer SHALL be sent trimmed, and an empty one refused with `Answer is empty`. A digit past the options, or `o` without a companion, SHALL answer nothing. Elsewhere, digits and `o` SHALL insert themselves. The block SHALL become its answered line, `▸ ✓ <message>  <option title or own answer>`. When point was in the block, it SHALL move to the end of the prompt area.

#### Scenario: A digit answers
- **WHEN** point is on a question's lines and `2` is typed
- **THEN** the agent gets `accept` with the choice set to the second option's value, and the block reads `▸ ✓ <message>  <second title>`

#### Scenario: RET answers
- **WHEN** point is on the `1` option's line and `RET` is typed
- **THEN** the agent gets `accept` with the first option's value and no prompt is sent

#### Scenario: Own answer to Claude
- **WHEN** `o` is typed on a Claude question and `Redis` is entered
- **THEN** the agent gets `accept` with `question_0_custom` set to `Redis` and no `question_0`

#### Scenario: Own answer to Codex
- **WHEN** `o` is typed on a Codex question and `qa` is entered
- **THEN** the agent gets `accept` with `env` set to `None of the above` and `env_note` set to `qa`

### Requirement: Multi-select questions
A multi-select question's option and own-answer rows SHALL read `[ ] ` before the title (`[x] ` once ticked), and its hint SHALL be `  <keys> tick   RET tick at point   C-c C-y send   C-c C-k skip`. On its lines a digit `1` to `9`, or `RET` on an option line, SHALL tick that option, or untick it when ticked, without answering. `o`, or `RET` on the own-answer line, SHALL read `Own answer: ` in the minibuffer, starting from the own answer's text when one is ticked, and tick the own answer with the trimmed text; an empty one SHALL untick it. A ticked own answer's row SHALL read `[x] Own answer  <text>`, the text's first line in `session-thought`. `C-c C-y` with point on the block's lines SHALL send it: `accept` with the choice field set to the ticked options' values, in option order, as a string array, and the own answer in the adapter's shape, the companion set to its text for a `_custom` companion, or `None of the above` added to the values and the companion set to its text for a `user_note` companion. The schema's `minItems` and `maxItems` SHALL NOT be drawn; when the count of ticked options plus a ticked own answer is below `minItems` or above `maxItems`, `C-c C-y` SHALL send nothing, keep the question open and echo `Pick at least <minItems>` or `Pick at most <maxItems>`. The block SHALL become its answered line, `▸ ✓ <message>  <titles and own answer, joined by ", ">`, or `▸ ✓ <message>  Nothing picked`.

#### Scenario: Ticking and sending
- **WHEN** point is on a multi-select question with options A, B and C, `1` and `3` are typed, then `C-c C-y`
- **THEN** after `1` the first row reads `[x] A` and nothing is sent, and the agent then gets `accept` with the choice set to `["a", "c"]`, and the block reads `▸ ✓ <message>  A, C`

#### Scenario: Unticking with RET
- **WHEN** the first option is ticked and `RET` is typed on its line
- **THEN** it reads `[ ] A` again and nothing is sent

#### Scenario: An own answer joins the picks
- **WHEN** `2` is typed, then `o` and `Redis`, then `C-c C-y` on a Claude multi-select question
- **THEN** the agent gets `accept` with `question_0` set to `["b"]` and `question_0_custom` set to `Redis`

#### Scenario: Out of range
- **WHEN** the field has `minItems` 2 and `maxItems` 2, one option is ticked and `C-c C-y` is typed, then three are ticked and `C-c C-y` is typed
- **THEN** the echo line says `Pick at least 2`, then `Pick at most 2`, nothing is sent and no count is drawn

#### Scenario: Editing the own answer
- **WHEN** `o` and `Redis` tick the own answer, then `o` is typed again
- **THEN** the row reads `[x] Own answer  Redis`, the minibuffer starts with `Redis`, and entering it empty unticks the own answer

### Requirement: Answering several questions
`TAB` with point on the block's lines SHALL open the next field, after the last or with none open the first, and move point to the column before its first option's chip. On the open field's lines, digits, `RET` and `o` SHALL act as on a single question's, except that answering a single-choice field SHALL record its answer, without sending, and open the next field. Answering the last SHALL fold every field, the header reading ` Questions  <a> of <n> answered`, `a` the answered fields, and the hint ` TAB open  C-c C-y send all  C-c C-k skip all`, with point on the last field's line; digits and `o` then answer nothing, and `TAB` opens the first field. A recorded own answer SHALL show on its row as `Own answer  <text>`, `o` SHALL start from it, and an empty one SHALL clear it. A multi-select field SHALL count as answered once an option or own answer is ticked, its answer the ticked titles joined by `, `. `C-c C-y` with point on the block's lines SHALL send `accept` with each answered field's content in its adapter's shape and no content for an unanswered one; when a ticked multi-select field is below its `minItems` or above its `maxItems`, it SHALL send nothing and echo `<title>: Pick at least <n>` or `<title>: Pick at most <n>`. The block SHALL become its answered line, `▸ ✓ <n> questions  <answer>` with each answered field's answer joined by ` · `, or `▸ ✓ <n> questions  Nothing answered`, `n` the block's fields. `C-c C-k` SHALL skip the whole block as it skips a question.

#### Scenario: A single choice moves on
- **WHEN** `2` is typed in the open `Status` field
- **THEN** nothing is sent, the block reads ` Questions  2 of 3`, ` ▸ ✓ Status  Both` and ` ▾ ● Tests   Which cases?` with Tests' rows, and point is before Tests' first chip

#### Scenario: TAB moves between questions
- **WHEN** point is in the block and `TAB` is typed on the last field
- **THEN** the first field opens and point moves to its first option

#### Scenario: Sending what is answered
- **WHEN** Claude's `question_0` is answered `Both`, `question_1` is not, and `C-c C-y` is typed in the block
- **THEN** the agent gets `accept` with `{"question_0": "Both"}` only, and the block reads `▸ ✓ 3 questions  Both`

#### Scenario: Skipping all
- **WHEN** `C-c C-k` is typed while the block is pending
- **THEN** the agent gets `decline` and the block reads `▸ – 3 questions  Skipped`

#### Scenario: Answering the last folds them all
- **WHEN** every field before `Commit` is answered and `1` is typed in the open `Commit` field
- **THEN** nothing is sent, the block reads ` Questions  3 of 3 answered`, a `▸ ✓` line per field and ` TAB open  C-c C-y send all  C-c C-k skip all`, and `TAB` then opens `Status`

### Requirement: Skipping, cancelling and withdrawing a question
`C-c C-k` SHALL answer the oldest pending request: a question with `decline`, its block becoming `▸ – <message>  Skipped`. `C-c C-c`, the end of the turn and killing the session buffer SHALL answer pending questions `cancel`, the block becoming `▸ ✗ <message>  Cancelled`; when the agent exits, the blocks become that line unanswered. A request the agent withdraws with `$/cancel_request`, question or permission request, SHALL close without a reply from the core: a question's block becoming `▸ ○ <message>  Closed`, a permission request's `○ withdrawn`.

#### Scenario: Skipping
- **WHEN** a question is pending and `C-c C-k` is typed
- **THEN** the agent gets `decline` and the block reads `▸ – <message>  Skipped`

#### Scenario: Cancelling
- **WHEN** a question is pending and `C-c C-c` is typed, or the turn ends, or the session buffer is killed
- **THEN** the agent gets `cancel` and the block reads `▸ ✗ <message>  Cancelled`

#### Scenario: Withdrawn
- **WHEN** the agent sends `$/cancel_request` for a pending question
- **THEN** its block reads `▸ ○ <message>  Closed`, the status no longer awaits an answer, and the agent gets the request-cancelled error

### Requirement: Non-blocking questions
A question whose request's `_meta.codex.delivery` is `async` SHALL be non-blocking. Its block SHALL show where it arrived, and the turn's later output SHALL go in below it; without a `toolCallId` it SHALL end the run of tool calls before it and join no group, so later calls group below it. It SHALL NOT move point; when the selected window shows the session, the echo line SHALL say `Question waiting: C-c C-q goes to it`. A non-blocking question of one field that is not a multi-select SHALL have the hint ` C-c C-q go to it   C-c C-k skip   an answer joins this turn`, its keys in `status-waiting` bold. It SHALL be answered, skipped and reached with `C-c C-q` like any question. When the turn ends, a non-blocking question still pending SHALL be answered `cancel` and its block SHALL become `▸ ○ <message>  Closed, the turn ended first`; one the agent withdraws first SHALL close as any withdrawn request does.

#### Scenario: Output goes in below an open question
- **GIVEN** a running turn
- **WHEN** a non-blocking question arrives, then an agent message
- **THEN** the message shows below the question's block, the block is still pending, and point stays in the prompt area

#### Scenario: Tool calls after an open question
- **GIVEN** a running turn with a non-blocking question pending
- **WHEN** three tool calls arrive
- **THEN** their group's row shows below the question's block

#### Scenario: The turn ends
- **WHEN** the turn ends with a non-blocking question pending
- **THEN** its block reads `▸ ○ <message>  Closed, the turn ended first` and the agent gets `cancel`

#### Scenario: Withdrawn at the turn's end
- **WHEN** the agent sends `$/cancel_request` for a pending non-blocking question, then ends the turn
- **THEN** its block reads `▸ ○ <message>  Closed`
