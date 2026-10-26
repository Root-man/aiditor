## ADDED Requirements

### Requirement: Tool calls show folded
Each `tool_call` update SHALL add a block above the prompt line, after any block being streamed, separated from the next by a blank line. Its first line, the header, SHALL read `<title> (<kind>, <status>)`, with the title's first line rendered as markdown and followed by ` …` when the title has more lines, ` (<kind>, <status>)` in the `dim` colour, and a rule in the `dim` colour beside it; `kind` and `status` are the ACP names (`execute`, `in_progress`). When the call has content, the header SHALL be followed by a newline and the content, which SHALL start folded: invisible, shown as ` …` at the header's end.

The content SHALL hold, in order and each separated by a blank line: `Input:` and the call's whole `rawInput` as pretty-printed JSON in a fenced `json` code block, when it has one; each content item, a text block as its markdown, a resource link as its URI, a diff as its path and a unified diff in a fenced `diff` code block, a terminal as `Terminal <id>`, and any other item as `[<type>]`; and `Output:` and the call's whole `rawOutput` in a fenced code block, when it has one and no content item: a string as itself, a list of text blocks as their text joined, anything else as pretty-printed JSON. No part SHALL be cut. A fence SHALL be longer than any run of backticks in what it encloses.

#### Scenario: A tool call starts folded
- **WHEN** the agent sends a `tool_call` titled `` `ls` `` of kind `execute`, status `pending` and `rawInput` `{"command":"ls"}`
- **THEN** a block reads `` `ls` (execute, pending) `` then its content, and the window shows only `ls (execute, pending) …`

#### Scenario: A call without content
- **WHEN** the agent sends a `tool_call` with no `rawInput`, content or `rawOutput`
- **THEN** its block is the header line alone, with no ` …`

#### Scenario: A multi-line title
- **WHEN** a call's title is `Run\nmore`
- **THEN** its header starts `Run …`

#### Scenario: Full output
- **WHEN** a call's `rawOutput` is a 50-line string and it has no content items, and its header is toggled open
- **THEN** all 50 lines show under `Output:`

### Requirement: Tool calls update in place
A `tool_call_update` SHALL merge its fields into the call with that id and rewrite the call's block where it is, even with later blocks after it, keeping whether it is folded; the rest of the transcript and the prompt area SHALL be unchanged. A `tool_call` with an id already shown SHALL replace that call the same way. An update for an unknown id SHALL start a block when it carries a title and be kept unrendered otherwise.

#### Scenario: Status updates
- **WHEN** a call is shown, the agent streams a message after it, then sends updates to `in_progress` and then `completed` with output
- **THEN** the call's header reads `(execute, completed)` above the message, and its content holds the output

#### Scenario: A repeated tool_call
- **WHEN** an unfolded call is sent again as a `tool_call` with the same id and status `completed`
- **THEN** its one block reads `(execute, completed)` and stays unfolded

#### Scenario: Point on the header
- **WHEN** point is on a call's header and an update changes its status
- **THEN** point is still on the header line

#### Scenario: An unknown id
- **WHEN** a `tool_call_update` arrives for an id never shown, without a title
- **THEN** the transcript is unchanged

### Requirement: TAB toggles a tool call
`TAB` in a session buffer (`agent-toggle-tool-call`) with point on a tool call's header line SHALL show its content if folded and hide it otherwise. With point anywhere in an unfolded call's content it SHALL hide the content and move point to the start of the header. Shown content SHALL be rendered as markdown, as messages are, with the rule beside each of its lines. With point anywhere else it SHALL echo `No tool call here`.

#### Scenario: Unfolding and folding
- **WHEN** point is on a folded call's header and `TAB` is typed, then `TAB` again
- **THEN** the content shows under the header with the rule beside it and its fences hidden, then hides again behind ` …`

#### Scenario: Folding from inside the content
- **WHEN** a call is unfolded, point is on a line of its content and `TAB` is typed
- **THEN** the content hides behind ` …` and point is at the start of the header

#### Scenario: Not on a header
- **WHEN** `TAB` is typed in the prompt area
- **THEN** `No tool call here` is echoed and nothing changes

#### Scenario: A call without content
- **WHEN** `TAB` is typed on the header of a call without content and an update then brings output
- **THEN** the output arrives folded

#### Scenario: Unfolded content stays open
- **WHEN** a call is unfolded and an update brings new output
- **THEN** the call stays unfolded, showing the new output

## MODIFIED Requirements

### Requirement: Streaming replies and turn ends
Each text agent message chunk SHALL be appended above the prompt line, consecutive chunks joining into one block, and each block separated from the next by a blank line. Each text agent thought chunk SHALL be appended the same way into a thought block; a message chunk after a thought chunk, or a thought chunk after a message chunk, SHALL start a new block. When the `session/prompt` answer arrives the turn SHALL end; a stop reason other than `end_turn` SHALL add the line `Turn ended: <reason>` (`max_tokens`, `max_turn_requests`, `refusal`, `cancelled`), and an error answer the line `Error: <message>`. Updates other than text chunks and tool calls SHALL stay kept and unrendered.

#### Scenario: A streamed reply
- **WHEN** `hello` is sent and the agent streams `Hi` then ` there` and ends the turn with `end_turn`
- **THEN** the buffer reads `> hello`, a blank line, `Hi there`, a blank line, then the prompt line, and `RET` can send again

#### Scenario: A thought between message chunks
- **WHEN** the agent streams the message chunk `Hi`, the thought chunk `hmm`, then the message chunk `there`
- **THEN** the transcript holds the blocks `Hi`, `hmm` and `there`, each followed by a blank line

#### Scenario: A refusal
- **WHEN** the turn ends with `refusal`
- **THEN** the line `Turn ended: refusal` appears above the prompt line

#### Scenario: An error answer
- **WHEN** the agent answers the prompt with an error `Internal error`
- **THEN** the line `Error: Internal error` appears above the prompt line and the turn ends
