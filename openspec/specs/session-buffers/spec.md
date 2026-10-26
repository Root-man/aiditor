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
Each text agent message chunk SHALL be appended above the prompt line, consecutive chunks joining into one block, and each block separated from the next by a blank line. Each text agent thought chunk SHALL be appended the same way into a thought block; a message chunk after a thought chunk, or a thought chunk after a message chunk, SHALL start a new block. When the `session/prompt` answer arrives the turn SHALL end; a stop reason other than `end_turn` SHALL add the line `Turn ended: <reason>` (`max_tokens`, `max_turn_requests`, `refusal`, `cancelled`), and an error answer the line `Error: <message>`. Updates other than text chunks, tool calls, plans and mode changes SHALL stay kept and unrendered.

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

### Requirement: Cancelling a turn
`C-c C-c` in a session buffer (`agent-cancel`) SHALL send `session/cancel` for the session while a turn runs; the turn ends when the agent answers the prompt, normally with `cancelled`. With no turn running it SHALL echo `No turn is running` and send nothing.

#### Scenario: Cancelling
- **WHEN** a turn runs, `C-c C-c` is typed and the agent ends the turn with `cancelled`
- **THEN** `session/cancel` is sent and `Turn ended: cancelled` appears above the prompt line

### Requirement: Messages render as markdown
Each agent message block, thought block and echoed prompt's text SHALL be rendered as markdown with text properties while its markup stays in the text: ATX and setext headings in the bold `text.title` colour, emphasis italic, strong emphasis bold, inline code in the `text.literal` colour on the `code_block` background, and link text and autolinks underlined in the `text.uri` colour. Heading markers, emphasis delimiters, code span backticks and a link's brackets and destination SHALL be invisible with no placeholder. Every line of a fenced or indented code block SHALL carry the `code_block` background decoration, and a code fence's text SHALL be invisible, its newline kept. A thought block SHALL be in the `dim` colour wherever no other colour applies. `Turn ended` and `Error` lines SHALL stay plain.

#### Scenario: Hidden markup
- **WHEN** the agent streams `# Title\n\nSome **bold**, *it*, `code` and [docs](https://x.dev).` and a frame is built
- **THEN** the lines read `Title`, an empty line, and `Some bold, it, code and docs.`, with `Title` bold, `bold` bold, `it` italic, `code` and `docs` coloured

#### Scenario: A code block
- **WHEN** the agent streams "```rust\nfn main() {}\n```" and a frame is built
- **THEN** the three lines carry the code block background and read empty, `fn main() {}` and empty

#### Scenario: A dim thought
- **WHEN** the agent streams the thought chunk `thinking` and a frame is built
- **THEN** `thinking` is drawn in the `dim` colour

#### Scenario: The echoed prompt
- **WHEN** `**hi** RET` is sent
- **THEN** the echo reads `> hi` with `hi` bold

### Requirement: Markup stays in the text
Rendering SHALL NOT change the buffer's text: search SHALL match hidden markup and copying or killing a rendered message SHALL yield its markdown.

#### Scenario: Swiper finds hidden markup
- **WHEN** a reply `**bold**` is rendered and swiper searches for `**b`
- **THEN** it matches the reply's line

#### Scenario: Copy yields markdown
- **WHEN** a rendered reply `**bold**` is marked whole and `M-w` is typed
- **THEN** the kill ring's top is `**bold**`

### Requirement: Streamed messages re-render once per frame
A streamed chunk's text SHALL be inserted at once, and the block being streamed SHALL be re-rendered when the next frame is built, once however many chunks arrived since the last frame. Only that block SHALL be re-rendered; earlier blocks SHALL keep their properties. A block that stops streaming SHALL be rendered before the next block starts.

#### Scenario: Chunks wait for the frame
- **WHEN** the chunks `**a` and `b**` arrive and no frame is built yet
- **THEN** the text reads `**ab**` and the block has no properties yet; after a frame is built `ab` is bold and the `**` hidden

#### Scenario: Earlier blocks are untouched
- **WHEN** a reply was rendered and another streams after the next prompt
- **THEN** building frames leaves the first reply's properties as they were

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

### Requirement: The plan shows and is replaced in place
The first `plan` update of a session SHALL add a block above the prompt line, after any block being streamed, separated from the next by a blank line. The block SHALL read `Plan`, then one line per entry in order: the entry's content's first line, followed by ` …` when it has more lines, then ` (<status>, <priority>)` in the `dim` colour, with the ACP names (`pending`, `in_progress`, `completed`; `high`, `medium`, `low`). Every line of the block SHALL have a rule in the `dim` colour beside it. Each later `plan` update SHALL replace the block's entries where the block is, even with later blocks after it; the rest of the transcript and the prompt area SHALL be unchanged. A plan with no entries SHALL show the `Plan` line alone.

#### Scenario: A plan shows
- **WHEN** the agent sends a plan with `Read the code` (`in_progress`, `high`) and `Write tests` (`pending`, `medium`)
- **THEN** a block reads `Plan`, `Read the code (in_progress, high)`, `Write tests (pending, medium)` above the prompt line

#### Scenario: A new plan replaces the old one in place
- **WHEN** a plan is shown, the agent streams a message after it, then sends a plan with `Read the code` `completed` and a third entry `Ship` (`pending`, `low`)
- **THEN** there is one plan block, above the message, reading `Plan`, `Read the code (completed, high)`, `Write tests (pending, medium)`, `Ship (pending, low)`

#### Scenario: An empty plan
- **WHEN** the agent sends a plan with no entries after a plan with entries
- **THEN** the plan block reads `Plan` alone

### Requirement: The mode line shows the current mode
A session buffer's mode line SHALL show ` [<value>]` after the buffer name once the session is ready and has a mode option (the first config option of category `mode`), its value being the option's current value. It SHALL follow each `config_option_update` and each `session/set_config_option` answer to the mode option's new current value, and show no mode when they no longer have a mode option. Before the session is ready, it SHALL show no mode.

#### Scenario: The mode at start
- **WHEN** the session becomes ready with a mode option whose current value is `auto`
- **THEN** the session buffer's mode line contains `[auto]`

#### Scenario: A mode change
- **WHEN** the agent sends `config_option_update` whose mode option's current value is `plan`
- **THEN** the session buffer's mode line contains `[plan]` and no longer `[auto]`

#### Scenario: Not ready
- **WHEN** the agent has not answered `session/new`
- **THEN** the session buffer's mode line shows no mode in brackets
