# agent-questions Specification

## Purpose
Questions from agents (`elicitation/create`): which requests the session buffer draws as a question, how a question shows, and how it is answered, skipped, cancelled and withdrawn.

## Requirements

### Requirement: Which requests are questions
A form-mode `elicitation/create` from a ready session SHALL be a question when its schema's properties are one or more choice fields, each with at most one companion field, and nothing else. A choice field is a string field with a non-empty `oneOf` (or `enum`, whose values are their own titles), a single-choice question, or an `array` field whose `items` have a non-empty `anyOf` (or `enum`), a multi-select question. A companion is a string field without choices whose key is its choice's key followed by `_custom` (claude-agent-acp), or whose `_meta.codex.role` is `user_note` and whose `_meta.codex.questionId` names its choice's key, or, with a single choice field, names nothing (codex-acp); a choice with a `user_note` companion needs some option besides codex's `None of the above`. Any other form, a URL-mode request, or a mode the client doesn't know SHALL be answered `decline` and echo `Can't show <agent>'s question yet; declined`, showing nothing in the session buffer. An `elicitation/create` from an ended or killed session SHALL be answered `cancel`; before the session is ready it SHALL get a "method not found" error.

#### Scenario: A Claude question
- **WHEN** the agent asks with properties `question_0` (string, `oneOf` A and B) and `question_0_custom` (string)
- **THEN** a question block shows with options A and B and an own-answer row

#### Scenario: A Claude multi-select question
- **WHEN** the agent asks with properties `question_0` (array, `items.anyOf` A and B) and `question_0_custom` (string)
- **THEN** a multi-select question block shows with options A and B and an own-answer row

#### Scenario: A Codex question
- **WHEN** the agent asks with properties `env` (string, `oneOf` `staging`, `preview` and `None of the above`) and `env_note` (string, `_meta.codex.role` `user_note`)
- **THEN** a question block shows with options `staging` and `preview` and an own-answer row, and no `None of the above` row

#### Scenario: Several Claude questions
- **WHEN** the agent asks with properties `question_0`, `question_0_custom`, `question_1` and `question_1_custom`
- **THEN** one block shows both questions, each with its own answer

#### Scenario: A form it can't draw
- **WHEN** the agent asks with a boolean field, or a string field without choices that is no choice's companion, or in URL mode
- **THEN** it is answered `decline`, the echo line says `Can't show fake's question yet; declined` and the session buffer is unchanged

### Requirement: A question shows above the prompt line
A question SHALL add a block above the prompt line, drawn as frame 6d, and stay pending. Every line of the block SHALL be indented one column and drawn on the `permission` background, with no rule. The lines are: ` Question  <message>` (its first line), with `Question` in `status-waiting` bold; then ` <title>` for the choice field's `title` when it has one that differs from the message; then a line per option, numbered from 1, of two spaces, the number centred in a three-column `key-hint` chip (three plain spaces past the ninth option), two spaces and its title, padded to the longest title and followed by two spaces and its `description` in `session-thought` when it has one; then the same line with the chip ` o ` and `Own answer` when the question has a companion; then the hint in `session-thought` with its keys in `status-waiting` bold: `  <keys> answer   RET answer with the row at point   C-c C-k skip`, or `  <keys> answer   C-n C-p preview another   C-c C-k skip` when an option has a preview, `<keys>` being `1` for one option and `1-<n>` for more, `n` at most 9. While point is on an option or own-answer line, that line SHALL also be drawn on the `region` background. While the request names a `toolCallId` of a call in a tool group, the block SHALL stand in for that call's row under the group's folded line, as a permission request's block does. The session's status SHALL show `● awaiting answer` in `status-waiting` while a question is pending and no permission request is.

#### Scenario: A question shows
- **WHEN** the agent asks `Where should the status go?` with options `Prompt` (`Replace the count`) and `Mode line`
- **THEN** ` Question  Where should the status go?\n   1   Prompt     Replace the count\n   2   Mode line\n  1-2 answer   RET answer with the row at point   C-c C-k skip` appears above the prompt line in the `permission` box and the status reads `● awaiting answer`

#### Scenario: The row at point
- **WHEN** point is on the `1` option's line
- **THEN** that line is drawn on the `region` background and the others are not

#### Scenario: More than nine options
- **WHEN** the agent asks with twelve options
- **THEN** the tenth to twelfth rows have no number in their chip, the hint starts `1-9 answer`, and `RET` on the tenth row answers it with the tenth option

### Requirement: Point and a waiting question
When a question shows and the session's prompt area is empty, point SHALL move to the column before its first option's chip in every window of the current workspace showing the session, and the buffer's own point, each such window scrolling so the block shows from its first line, the prompt line too when it fits. When the prompt area holds text, point SHALL stay, keys SHALL keep typing into the prompt, and, when the selected window shows the session, the echo line SHALL say `Question waiting: C-c C-q goes to it`. `C-c C-q` (`agent-goto-question`) SHALL move point to the column before the oldest pending question's first chip, scrolling so the block shows from its first line, or echo `No question is pending`.

#### Scenario: An empty prompt
- **WHEN** a question arrives while the prompt area is empty
- **THEN** point is just before its ` 1 ` chip

#### Scenario: A question at the bottom of a long transcript
- **WHEN** a question arrives while the window shows the end of a transcript longer than it
- **THEN** the window shows the block from ` Question` to its hint row

#### Scenario: A prompt with text
- **WHEN** a question arrives while `draft` is typed in the prompt area, then `1` is typed and `C-c C-q`
- **THEN** the echo line said `Question waiting: C-c C-q goes to it`, the prompt reads `draft1`, and point is then just before the question's ` 1 ` chip

### Requirement: Answering a question
On a pending question's lines, a digit `1` to `9` SHALL answer it with that option at once, and `RET` on an option line SHALL answer it with that option; the answer SHALL be `accept` with the choice field set to the option's value. `o` on a question's lines SHALL read `Own answer: ` in the minibuffer when the question has a companion, and answer `accept` in the adapter's shape: the companion alone for a `_custom` companion, the choice field set to `None of the above` and the companion to the text for a `user_note` companion. An own answer SHALL be sent trimmed, and an empty one refused with `Answer is empty`. A digit past the options, or `o` without a companion, SHALL answer nothing. Elsewhere, digits and `o` SHALL insert themselves. The block SHALL become one line in `session-thought` with no rule: `✓ <message>  <option title or own answer>`. When point was in the block, it SHALL move to the end of the prompt area.

#### Scenario: A digit answers
- **WHEN** point is on a question's lines and `2` is typed
- **THEN** the agent gets `accept` with the choice set to the second option's value, and the block reads `✓ <message>  <second title>`

#### Scenario: RET answers
- **WHEN** point is on the `1` option's line and `RET` is typed
- **THEN** the agent gets `accept` with the first option's value and no prompt is sent

#### Scenario: Own answer to Claude
- **WHEN** `o` is typed on a Claude question and `Redis` is entered
- **THEN** the agent gets `accept` with `question_0_custom` set to `Redis` and no `question_0`

#### Scenario: Own answer to Codex
- **WHEN** `o` is typed on a Codex question and `qa` is entered
- **THEN** the agent gets `accept` with `env` set to `None of the above` and `env_note` set to `qa`

### Requirement: Skipping, cancelling and withdrawing a question
`C-c C-k` SHALL answer the oldest pending request: a question with `decline`, its block becoming `– <message>  Skipped`. `C-c C-c`, the end of the turn and killing the session buffer SHALL answer pending questions `cancel`, the block becoming `✗ <message>  Cancelled`; when the agent exits, the blocks become that line unanswered. A request the agent withdraws with `$/cancel_request`, question or permission request, SHALL close its block as `○ withdrawn` without a reply from the core.

#### Scenario: Skipping
- **WHEN** a question is pending and `C-c C-k` is typed
- **THEN** the agent gets `decline` and the block reads `– <message>  Skipped`

#### Scenario: Cancelling
- **WHEN** a question is pending and `C-c C-c` is typed, or the turn ends, or the session buffer is killed
- **THEN** the agent gets `cancel`

#### Scenario: Withdrawn
- **WHEN** the agent sends `$/cancel_request` for a pending question
- **THEN** its block reads `○ withdrawn`, the status no longer awaits an answer, and the agent gets the request-cancelled error

### Requirement: Multi-select questions
A multi-select question's option and own-answer rows SHALL read `[ ] ` before the title (`[x] ` once ticked), and its hint SHALL be `  <keys> tick   RET tick at point   C-c C-y send   C-c C-k skip`. On its lines a digit `1` to `9`, or `RET` on an option line, SHALL tick that option, or untick it when ticked, without answering. `o`, or `RET` on the own-answer line, SHALL read `Own answer: ` in the minibuffer, starting from the own answer's text when one is ticked, and tick the own answer with the trimmed text; an empty one SHALL untick it. A ticked own answer's row SHALL read `[x] Own answer  <text>`, the text's first line in `session-thought`. `C-c C-y` with point on the block's lines SHALL send it: `accept` with the choice field set to the ticked options' values, in option order, as a string array, and the own answer in the adapter's shape, the companion set to its text for a `_custom` companion, or `None of the above` added to the values and the companion set to its text for a `user_note` companion. The schema's `minItems` and `maxItems` SHALL NOT be drawn; when the count of ticked options plus a ticked own answer is below `minItems` or above `maxItems`, `C-c C-y` SHALL send nothing, keep the question open and echo `Pick at least <minItems>` or `Pick at most <maxItems>`. The block SHALL become `✓ <message>  <titles and own answer, joined by ", ">`, or `✓ <message>  Nothing picked`.

#### Scenario: Ticking and sending
- **WHEN** point is on a multi-select question with options A, B and C, `1` and `3` are typed, then `C-c C-y`
- **THEN** after `1` the first row reads `[x] A` and nothing is sent, and the agent then gets `accept` with the choice set to `["a", "c"]`, and the block reads `✓ <message>  A, C`

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

### Requirement: Option previews
An option carrying `_meta["_claude/askUserQuestionOption"].preview` SHALL show its preview under its row while a window's point is on that row, each window following its own point, and the preview SHALL NOT be part of the buffer's text. Each preview line SHALL be in the block's box, starting under the option's title, its text on the `session-block` background. A preview of more than 8 lines SHALL show its first 7 and `… <n> more lines` in `session-thought`, `n` the lines left out. Moving point to another option's row SHALL show that option's preview instead, and an option without one shows none.

#### Scenario: The preview follows point
- **WHEN** a question's options 1 and 2 have previews `one` and `two` and point moves from row 1 to row 2 with `C-n`
- **THEN** `one` is drawn under row 1 and then `two` under row 2, `one` no longer drawn

#### Scenario: A long preview folds
- **WHEN** the option at point has a 12-line preview
- **THEN** its first 7 lines are drawn under its row, then `… 5 more lines`

### Requirement: Several questions in one block
A question with more than one choice field SHALL show as one block, drawn as frame 6e, in the same box: ` Questions  <k> of <n>` with `Questions` in `status-waiting` bold, `k` the open field's place; then a line per field in key order, a trailing number compared as a number; then the hint. The open field's line SHALL read ` ▾ ● <title>  <description>`, `●` in `status-waiting`, followed by its option and own-answer rows as a single question draws them, three columns further in. Every other field's line SHALL read ` ▸ ✓ <title>  <answer>` once answered, `✓` in `status-running` and the answer in `session-thought`, or ` ▸ ○ <title>  <description>` until then, `▸ ○` and the description in `session-thought`. Titles SHALL be padded to the longest, a field without a title being named by its key. The hint SHALL read ` <keys> answer  TAB next  C-c C-y send all  C-c C-k skip all`, `tick` in place of `answer` when the open field is a multi-select. The first field SHALL be open when the block shows.

#### Scenario: Several questions show
- **WHEN** the agent asks fields `Status` (options `Prompt`, `Both`), `Tests` (description `Which cases?`) and `Commit` (description `One or many?`)
- **THEN** the block reads ` Questions  1 of 3`, ` ▾ ● Status`, its option rows, ` ▸ ○ Tests   Which cases?`, ` ▸ ○ Commit  One or many?` and ` 1-2 answer  TAB next  C-c C-y send all  C-c C-k skip all`, and point is before its first chip

### Requirement: Answering several questions
`TAB` with point on the block's lines SHALL open the next field, after the last or with none open the first, and move point to the column before its first option's chip. On the open field's lines, digits, `RET` and `o` SHALL act as on a single question's, except that answering a single-choice field SHALL record its answer, without sending, and open the next field. Answering the last SHALL fold every field, the header reading ` Questions  <a> of <n> answered`, `a` the answered fields, and the hint ` TAB open  C-c C-y send all  C-c C-k skip all`, with point on the last field's line; digits and `o` then answer nothing, and `TAB` opens the first field. A recorded own answer SHALL show on its row as `Own answer  <text>`, `o` SHALL start from it, and an empty one SHALL clear it. A multi-select field SHALL count as answered once an option or own answer is ticked, its answer the ticked titles joined by `, `. `C-c C-y` with point on the block's lines SHALL send `accept` with each answered field's content in its adapter's shape and no content for an unanswered one; when a ticked multi-select field is below its `minItems` or above its `maxItems`, it SHALL send nothing and echo `<title>: Pick at least <n>` or `<title>: Pick at most <n>`. The block SHALL become `✓ <message>  <title>: <answer>` for each answered field, joined by `; `, or `✓ <message>  Nothing answered`. `C-c C-k` SHALL skip the whole block as it skips a question.

#### Scenario: A single choice moves on
- **WHEN** `2` is typed in the open `Status` field
- **THEN** nothing is sent, the block reads ` Questions  2 of 3`, ` ▸ ✓ Status  Both` and ` ▾ ● Tests   Which cases?` with Tests' rows, and point is before Tests' first chip

#### Scenario: TAB moves between questions
- **WHEN** point is in the block and `TAB` is typed on the last field
- **THEN** the first field opens and point moves to its first option

#### Scenario: Sending what is answered
- **WHEN** Claude's `question_0` is answered `Both`, `question_1` is not, and `C-c C-y` is typed in the block
- **THEN** the agent gets `accept` with `{"question_0": "Both"}` only, and the block reads `✓ <message>  Status: Both`

#### Scenario: Skipping all
- **WHEN** `C-c C-k` is typed while the block is pending
- **THEN** the agent gets `decline` and the block reads `– <message>  Skipped`

#### Scenario: Answering the last folds them all
- **WHEN** every field before `Commit` is answered and `1` is typed in the open `Commit` field
- **THEN** nothing is sent, the block reads ` Questions  3 of 3 answered`, a `▸ ✓` line per field and ` TAB open  C-c C-y send all  C-c C-k skip all`, and `TAB` then opens `Status`

### Requirement: Non-blocking questions
A question whose request's `_meta.codex.delivery` is `async` SHALL be non-blocking. Its block SHALL show where it arrived, and the turn's later output SHALL go in below it; without a `toolCallId` it SHALL end the run of tool calls before it and join no group, so later calls group below it. It SHALL NOT move point; when the selected window shows the session, the echo line SHALL say `Question waiting: C-c C-q goes to it`. A non-blocking question of one field that is not a multi-select SHALL have the hint ` C-c C-q go to it   C-c C-k skip   an answer joins this turn`, its keys in `status-waiting` bold. It SHALL be answered, skipped and reached with `C-c C-q` like any question. When the turn ends, a non-blocking question still pending SHALL be answered `cancel` and its block SHALL become `○ withdrawn`; one the agent withdraws first SHALL close as any withdrawn request does.

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
- **THEN** its block reads `○ withdrawn` and the agent gets `cancel`

#### Scenario: Withdrawn at the turn's end
- **WHEN** the agent sends `$/cancel_request` for a pending non-blocking question, then ends the turn
- **THEN** its block reads `○ withdrawn`
