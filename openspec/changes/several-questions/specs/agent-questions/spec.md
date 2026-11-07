## MODIFIED Requirements

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

## ADDED Requirements

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
