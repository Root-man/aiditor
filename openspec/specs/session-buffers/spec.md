# session-buffers Specification

## Purpose
The session buffer: one comint-style buffer per agent session holding the read-only transcript of the conversation above a prompt area where the user writes, sends and cancels turns.

## Requirements

### Requirement: The session buffer
`agent-start` SHALL create a buffer named `*<agent>*<<workspace>>`, made unique, owned by the current workspace, in `agent-session-mode`, and show it in the selected window. Its text SHALL start as the prompt line `> ` with point after it.

#### Scenario: Starting shows the session buffer
- **WHEN** `M-x agent-start RET claude RET` is typed in workspace `work`
- **THEN** the selected window shows `*claude*<work>`, whose text is `> `

### Requirement: Read-only transcript, editable prompt area
Text before the prompt area SHALL be read-only: an edit that inserts before the prompt area's start or deletes any of the text before it SHALL be refused with `Text is read-only` and change nothing. The prompt area, from after the `> ` prompt line to the end of the buffer, SHALL edit like any buffer, undo included. Output SHALL be inserted above the prompt line, moving the prompt area down and keeping its text and undo.

#### Scenario: Editing the transcript is refused
- **WHEN** point is in the transcript and a char is typed
- **THEN** the echo area shows `Text is read-only` and the text is unchanged

#### Scenario: The prompt area edits
- **WHEN** `ab DEL c` is typed in the prompt area
- **THEN** the prompt area holds `ac`, and `C-/` undoes the last edit

#### Scenario: Output keeps the prompt area
- **WHEN** a message chunk arrives while the prompt area holds `draft`
- **THEN** the chunk appears above the prompt line and the prompt area still holds `draft`

### Requirement: Sending a prompt
`RET` in a session buffer (`agent-send-input`) SHALL, when the session is ready and no turn runs, send the prompt area's text as one text block in a `session/prompt`, leave the text above the prompt area as the echo (`> ` and the text), start a new empty prompt line below it with point there, and mark a turn running. Input that is empty or only whitespace SHALL send nothing. While a turn runs `RET` SHALL echo `<agent> is busy` and send nothing; before the session is ready it SHALL echo `<agent> is not ready`, and after the session ended `<agent> session has ended`. `C-j` and `S-RET` SHALL insert a newline.

#### Scenario: A prompt is sent
- **WHEN** the session is ready and `hello RET` is typed
- **THEN** a `session/prompt` with the text `hello` is sent and the buffer reads `> hello` above an empty prompt line

#### Scenario: Busy
- **WHEN** a turn runs and `again RET` is typed
- **THEN** the echo area shows `claude is busy`, nothing is sent and the prompt area still holds `again`

#### Scenario: A newline
- **WHEN** `a C-j b S-RET c` is typed in the prompt area
- **THEN** the prompt area holds `a\nb\nc` and nothing is sent

### Requirement: Streaming replies and turn ends
Each text agent message chunk SHALL be appended above the prompt line as plain text, consecutive chunks joining into one block, and each block separated from the next by a blank line. When the `session/prompt` answer arrives the turn SHALL end; a stop reason other than `end_turn` SHALL add the line `Turn ended: <reason>` (`max_tokens`, `max_turn_requests`, `refusal`, `cancelled`), and an error answer the line `Error: <message>`. Other updates SHALL stay kept and unrendered.

#### Scenario: A streamed reply
- **WHEN** `hello` is sent and the agent streams `Hi` then ` there` and ends the turn with `end_turn`
- **THEN** the buffer reads `> hello`, a blank line, `Hi there`, a blank line, then the prompt line, and `RET` can send again

#### Scenario: A refusal
- **WHEN** the turn ends with `refusal`
- **THEN** the line `Turn ended: refusal` appears above the prompt line

#### Scenario: An error answer
- **WHEN** the agent answers the prompt with an error `Internal error`
- **THEN** the line `Error: Internal error` appears above the prompt line and the turn ends

### Requirement: Cancelling a turn
`C-c C-c` in a session buffer (`agent-cancel`) SHALL send `session/cancel` for the session while a turn runs; the turn ends when the agent answers the prompt, normally with `cancelled`. With no turn running it SHALL echo `No turn is running` and send nothing.

#### Scenario: Cancelling
- **WHEN** a turn runs, `C-c C-c` is typed and the agent ends the turn with `cancelled`
- **THEN** `session/cancel` is sent and `Turn ended: cancelled` appears above the prompt line
