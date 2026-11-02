# Spec Delta

## ADDED Requirements

### Requirement: Messages are units of the transcript
The session SHALL keep its messages in transcript order: each agent message block and user message block, with the ACP `messageId` its chunks carried, for live and replayed chunks alike. A prompt sent with `RET` SHALL be a message with no id, placed after the last agent message before it. Each block SHALL be one message, so a `messageId` continuing after a tool call is a second message with the same id. Thoughts, tool calls, plans, permission requests and turn-end lines SHALL NOT be messages.

#### Scenario: Ids are kept
- **WHEN** the agent streams `a` and `b` with id `m1`, then `c` with id `m2`
- **THEN** the session holds two messages, `ab` with id `m1` and `c` with id `m2`

#### Scenario: A replayed user message keeps its id
- **WHEN** a `user_message_chunk` with id `u1` arrives, then an agent chunk with id `m1`
- **THEN** the session holds the messages `u1` then `m1`, in that order

#### Scenario: A sent prompt has no id
- **WHEN** an agent message with id `m1` is shown and `hello` is sent
- **THEN** the session's messages are `m1`, then one without an id for `> hello`

### Requirement: The message at point is marked
A window showing a session buffer SHALL draw a bar in the left margin in the theme's `message-at-point` foreground beside every row of the message at point: the last message starting at or before that window's point. Point on a tool call, a blank line or in the prompt area SHALL leave the bar on the message before it. With no message at or before point there SHALL be no bar. A streaming message SHALL be marked to its current end.

#### Scenario: The bar follows point
- **WHEN** the transcript holds a two-line message `a` then a message `b`, and point moves from `b` to `a`
- **THEN** the bar leaves `b`'s line and is beside both of `a`'s lines

#### Scenario: Point past a tool call
- **WHEN** point is in the prompt area and a tool call follows the last message
- **THEN** the bar is beside the last message and not beside the tool call

#### Scenario: Two windows
- **WHEN** two windows show a session with point in different messages
- **THEN** each window marks its own point's message

### Requirement: Moving between messages
`C-c C-n` in a session buffer (`agent-next-message`) SHALL move point to the start of the nearest message starting after point, and `C-c C-p` (`agent-previous-message`) to the start of the nearest one starting before point, skipping tool calls and thoughts. A prefix count SHALL move that many messages, stopping at the farthest, and a negative count SHALL move the other way. With no message that way they SHALL echo `No message found` and leave point.

#### Scenario: Past tool calls
- **WHEN** a message, a tool call, a thought and a second message are shown, with point in the prompt area, and `C-c C-p` is typed twice, then `C-c C-n`
- **THEN** point lands on the second message, the first, then the second

#### Scenario: The middle of a message
- **WHEN** point is inside a message and `C-c C-p` is typed
- **THEN** point moves to that message's start

#### Scenario: No message that way
- **WHEN** point is on the last message's start and `C-c C-n` is typed
- **THEN** `No message found` is echoed and point stays

## MODIFIED Requirements

### Requirement: Streaming replies and turn ends
Each text agent message chunk SHALL be appended above the prompt line, consecutive chunks with the same `messageId` joining into one block, and each block separated from the next by a blank line. Each text agent thought chunk SHALL be appended the same way into a thought block; a message chunk after a thought chunk, a thought chunk after a message chunk, or a chunk with a different `messageId` SHALL start a new block. When the `session/prompt` answer arrives the turn SHALL end; a stop reason other than `end_turn` SHALL add the line `Turn ended: <reason>` (`max_tokens`, `max_turn_requests`, `refusal`, `cancelled`), and an error answer the line `Error: <message>`. Updates other than text chunks, tool calls, plans and mode changes SHALL stay kept and unrendered.

#### Scenario: A streamed reply
- **WHEN** `hello` is sent and the agent streams `Hi` then ` there` and ends the turn with `end_turn`
- **THEN** the buffer reads `> hello`, a blank line, `Hi there`, a blank line, then the prompt line, and `RET` can send again

#### Scenario: A thought between message chunks
- **WHEN** the agent streams the message chunk `Hi`, the thought chunk `hmm`, then the message chunk `there`
- **THEN** the transcript holds the blocks `Hi`, `hmm` and `there`, each followed by a blank line

#### Scenario: A new message id
- **WHEN** the agent streams `Hi` with id `m1`, then `there` with id `m2`
- **THEN** the transcript holds the blocks `Hi` and `there`, each followed by a blank line

#### Scenario: A refusal
- **WHEN** the turn ends with `refusal`
- **THEN** the line `Turn ended: refusal` appears above the prompt line

#### Scenario: An error answer
- **WHEN** the agent answers the prompt with an error `Internal error`
- **THEN** the line `Error: Internal error` appears above the prompt line and the turn ends

### Requirement: Answering a permission request
`C-c C-y` in a session buffer (`agent-allow-once`) SHALL answer the oldest pending request with its first `allow_once` option, and `C-c C-k` (`agent-reject-once`) with its first `reject_once` option, wherever point is. `RET` on an option line of any pending request SHALL answer it with that option; on any other line `RET` sends the prompt as before. The answer SHALL be the `selected` outcome with the option's id, and the request's block SHALL become the single line `Chose <name> (<kind>) for <title>`. With no pending request `C-c C-y` and `C-c C-k` SHALL echo `No permission request is pending`; when the oldest has no option of the kind they SHALL echo `No allow_once option for <title>` (or `reject_once`) and answer nothing.

#### Scenario: Allowing
- **WHEN** a request for `Edit a.txt` is pending and `C-c C-y` is typed in the prompt area
- **THEN** the `Yes` option is selected and the block reads `Chose Yes (allow_once) for Edit a.txt`

#### Scenario: Rejecting
- **WHEN** the same request is pending and `C-c C-k` is typed
- **THEN** the `No` option is selected and the block reads `Chose No (reject_once) for Edit a.txt`

#### Scenario: RET on an option
- **WHEN** point is on the line `2. No (reject_once)` and `RET` is typed
- **THEN** the `No` option is selected and no prompt is sent

#### Scenario: Oldest first
- **WHEN** requests for `A` and then `B` are pending and `C-c C-y` is typed twice
- **THEN** `A` is answered first, then `B`

#### Scenario: Nothing to answer
- **WHEN** no request is pending, or the oldest has no `allow_once` option, and `C-c C-y` is typed
- **THEN** `No permission request is pending`, or `No allow_once option for <title>`, is echoed and nothing is answered
