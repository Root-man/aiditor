# Spec Delta

## Purpose
Questions from agents (`elicitation/create`): which requests the session buffer draws as a question, how a question shows, and how it is answered, skipped, cancelled and withdrawn.

## ADDED Requirements

### Requirement: Which requests are questions
A form-mode `elicitation/create` from a ready session SHALL be a single-choice question when its schema's properties are exactly one choice field, a string field with a non-empty `oneOf` (or `enum`, whose values are their own titles), and at most one companion field: a string field without choices whose key is the choice's key followed by `_custom` (claude-agent-acp), or whose `_meta.codex.role` is `user_note` (codex-acp), and some option besides codex's `None of the above`. Any other form, a URL-mode request, or a mode the client doesn't know SHALL be answered `decline` and echo `Can't show <agent>'s question yet; declined`, showing nothing in the session buffer. An `elicitation/create` from an ended or killed session SHALL be answered `cancel`; before the session is ready it SHALL get a "method not found" error.

#### Scenario: A Claude question
- **WHEN** the agent asks with properties `question_0` (string, `oneOf` A and B) and `question_0_custom` (string)
- **THEN** a question block shows with options A and B and an own-answer row

#### Scenario: A Codex question
- **WHEN** the agent asks with properties `env` (string, `oneOf` `staging`, `preview` and `None of the above`) and `env_note` (string, `_meta.codex.role` `user_note`)
- **THEN** a question block shows with options `staging` and `preview` and an own-answer row, and no `None of the above` row

#### Scenario: A form it can't draw
- **WHEN** the agent asks with two choice fields, or a boolean field, or in URL mode
- **THEN** it is answered `decline`, the echo line says `Can't show fake's question yet; declined` and the session buffer is unchanged

### Requirement: A question shows above the prompt line
A question SHALL add a block above the prompt line, drawn as frame 6d, and stay pending. Every line of the block SHALL be indented one column and drawn on the `permission` background, with no rule. The lines are: ` Question  <message>` (its first line), with `Question` in `status-waiting` bold; then ` <title>` for the choice field's `title` when it has one that differs from the message; then a line per option, numbered from 1, of two spaces, the number centred in a three-column `key-hint` chip, two spaces and its title, padded to the longest title and followed by two spaces and its `description` in `session-thought` when it has one; then the same line with the chip ` o ` and `Own answer` when the question has a companion; then `  <keys> answer   RET answer with the row at point   C-c C-k skip` in `session-thought` with `<keys>`, `RET` and `C-c C-k` in `status-waiting` bold, `<keys>` being `1` for one option and `1-<n>` for more. While point is on an option or own-answer line, that line SHALL also be drawn on the `region` background. While the request names a `toolCallId` of a call in a tool group, the block SHALL stand in for that call's row under the group's folded line, as a permission request's block does. The session's status SHALL show `● awaiting answer` in `status-waiting` while a question is pending and no permission request is.

#### Scenario: A question shows
- **WHEN** the agent asks `Where should the status go?` with options `Prompt` (`Replace the count`) and `Mode line`
- **THEN** ` Question  Where should the status go?\n   1   Prompt     Replace the count\n   2   Mode line\n  1-2 answer   RET answer with the row at point   C-c C-k skip` appears above the prompt line in the `permission` box and the status reads `● awaiting answer`

#### Scenario: The row at point
- **WHEN** point is on the `1` option's line
- **THEN** that line is drawn on the `region` background and the others are not

### Requirement: Point and a waiting question
When a question shows and the session's prompt area is empty, point SHALL move to the column before its first option's chip in every window of the current workspace showing the session, and the buffer's own point. When the prompt area holds text, point SHALL stay, keys SHALL keep typing into the prompt, and, when the selected window shows the session, the echo line SHALL say `Question waiting: C-c C-q goes to it`. `C-c C-q` (`agent-goto-question`) SHALL move point to the column before the oldest pending question's first chip, or echo `No question is pending`.

#### Scenario: An empty prompt
- **WHEN** a question arrives while the prompt area is empty
- **THEN** point is just before its ` 1 ` chip

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
