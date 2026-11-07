## MODIFIED Requirements

### Requirement: Which requests are questions
A form-mode `elicitation/create` from a ready session SHALL be a question when its schema's properties are exactly one choice field and at most one companion field. A choice field is a string field with a non-empty `oneOf` (or `enum`, whose values are their own titles), a single-choice question, or an `array` field whose `items` have a non-empty `anyOf` (or `enum`), a multi-select question. A companion is a string field without choices whose key is the choice's key followed by `_custom` (claude-agent-acp), or whose `_meta.codex.role` is `user_note` (codex-acp), and some option besides codex's `None of the above`. Any other form, a URL-mode request, or a mode the client doesn't know SHALL be answered `decline` and echo `Can't show <agent>'s question yet; declined`, showing nothing in the session buffer. An `elicitation/create` from an ended or killed session SHALL be answered `cancel`; before the session is ready it SHALL get a "method not found" error.

#### Scenario: A Claude question
- **WHEN** the agent asks with properties `question_0` (string, `oneOf` A and B) and `question_0_custom` (string)
- **THEN** a question block shows with options A and B and an own-answer row

#### Scenario: A Claude multi-select question
- **WHEN** the agent asks with properties `question_0` (array, `items.anyOf` A and B) and `question_0_custom` (string)
- **THEN** a multi-select question block shows with options A and B and an own-answer row

#### Scenario: A Codex question
- **WHEN** the agent asks with properties `env` (string, `oneOf` `staging`, `preview` and `None of the above`) and `env_note` (string, `_meta.codex.role` `user_note`)
- **THEN** a question block shows with options `staging` and `preview` and an own-answer row, and no `None of the above` row

#### Scenario: A form it can't draw
- **WHEN** the agent asks with two choice fields, or a boolean field, or in URL mode
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

## ADDED Requirements

### Requirement: Multi-select questions
A multi-select question's option and own-answer rows SHALL read `[ ] ` before the title (`[x] ` once ticked), and its hint SHALL be `  <keys> tick   RET tick at point   C-c C-y send   C-c C-k skip`. On its lines a digit `1` to `9`, or `RET` on an option line, SHALL tick that option, or untick it when ticked, without answering. `o`, or `RET` on the own-answer line, SHALL read `Own answer: ` in the minibuffer and tick the own answer with the trimmed text, an empty one refused with `Answer is empty`; when the own answer is ticked they SHALL untick it instead. `C-c C-y` with point on the block's lines SHALL send it: `accept` with the choice field set to the ticked options' values, in option order, as a string array, and the own answer in the adapter's shape, the companion set to its text for a `_custom` companion, or `None of the above` added to the values and the companion set to its text for a `user_note` companion. The schema's `minItems` and `maxItems` SHALL NOT be drawn; when the count of ticked options plus a ticked own answer is below `minItems` or above `maxItems`, `C-c C-y` SHALL send nothing, keep the question open and echo `Pick at least <minItems>` or `Pick at most <maxItems>`. The block SHALL become `✓ <message>  <titles and own answer, joined by ", ">`, or `✓ <message>  Nothing picked`.

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

### Requirement: Option previews
An option carrying `_meta["_claude/askUserQuestionOption"].preview` SHALL show its preview under its row while a window's point is on that row, each window following its own point, and the preview SHALL NOT be part of the buffer's text. Each preview line SHALL be in the block's box, starting under the option's title, its text on the `session-block` background. A preview of more than 8 lines SHALL show its first 7 and `… <n> more lines` in `session-thought`, `n` the lines left out. Moving point to another option's row SHALL show that option's preview instead, and an option without one shows none.

#### Scenario: The preview follows point
- **WHEN** a question's options 1 and 2 have previews `one` and `two` and point moves from row 1 to row 2 with `C-n`
- **THEN** `one` is drawn under row 1 and then `two` under row 2, `one` no longer drawn

#### Scenario: A long preview folds
- **WHEN** the option at point has a 12-line preview
- **THEN** its first 7 lines are drawn under its row, then `… 5 more lines`
