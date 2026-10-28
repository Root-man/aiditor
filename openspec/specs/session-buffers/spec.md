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
Each `tool_call` update SHALL add a block above the prompt line, after any block being streamed, separated from the next by a blank line. Its first line, the header, SHALL read `<title> (<kind>, <status>)`, or `<title> (<kind>, <status>, <exit>)` for a failed command (see Command output shows in its tool call), with the title's first line rendered as markdown and followed by ` …` when the title has more lines, ` (<kind>, <status>)` in the `dim` colour, and a rule in the `dim` colour beside it; `kind` and `status` are the ACP names (`execute`, `in_progress`). When the call has content, the header SHALL be followed by a newline and the content, which SHALL start folded: invisible, shown as ` …` at the header's end.

The content SHALL hold, in order and each separated by a blank line: `Input:` and the call's whole `rawInput` as pretty-printed JSON in a fenced `json` code block, when it has one; each content item, a text block as its markdown, a resource link as its URI, a diff as its path and a unified diff in a fenced `diff` code block, a terminal other than the call's own terminal (see Command output shows in its tool call) as `Terminal <id>`, and any other item as `[<type>]`; and `Output:` in a fenced code block, holding the call's terminal output when it has a terminal, else the call's whole `rawOutput` when it has one and no content item: a string as itself, a list of text blocks as their text joined, anything else as pretty-printed JSON. No part SHALL be cut. A fence SHALL be longer than any run of backticks in what it encloses.

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

### Requirement: S-TAB cycles the session's mode
`S-TAB` in a session buffer (`agent-cycle-mode`) SHALL send `session/set_config_option` setting the session's mode option to the value after its current one in the option's order (groups flattened), wrapping from the last value to the first. The values and current value are those of the latest `session/new` answer, `config_option_update` or `session/set_config_option` answer. It SHALL work while a turn runs. The mode line SHALL follow the answer. A session without a mode option SHALL echo `<agent> has no modes`; before `session/new` is answered it SHALL echo `<agent> is not ready`, and after the agent exits `<agent> session has ended`; in these cases nothing is sent. If the agent answers with an error, `*Messages*` SHALL get `<agent> could not set mode <value>: <reason>` with the value sent, and the mode line keeps its mode.

#### Scenario: Cycling through every mode
- **WHEN** the session's mode option is `auto` with values `auto` and `plan`, and `S-TAB` is typed, the agent answers, and `S-TAB` is typed again and answered
- **THEN** the first sends `plan` and the mode line shows `[plan]`, the second sends `auto` and the mode line shows `[auto]`

#### Scenario: Following the agent's own change
- **WHEN** the agent sends `config_option_update` setting the mode to `plan`, then `S-TAB` is typed
- **THEN** `auto` is sent

#### Scenario: No mode option
- **WHEN** the session has no mode option and `S-TAB` is typed
- **THEN** `claude has no modes` is echoed and nothing is sent

#### Scenario: Not ready or ended
- **WHEN** `S-TAB` is typed before `session/new` is answered, or after the agent exited
- **THEN** `claude is not ready`, or `claude session has ended`, is echoed and nothing is sent

#### Scenario: A refused cycle
- **WHEN** the mode is `plan` and the agent answers the cycle to `auto` with an error
- **THEN** `*Messages*` has `claude could not set mode auto: ` followed by the reason and the mode line keeps `[plan]`

### Requirement: Permission requests show inline
A `session/request_permission` from a session whose `session/new` was answered and whose agent hasn't exited SHALL add a block above the prompt line and stay pending: the line `Permission: <title>`, then a line per option in order, `<n>. <name> (<kind>)`, numbered from 1, with the ACP name of the kind in the `dim` colour and a `dim` rule beside every line of the block. The title is the first line of the request's tool call title (followed by ` …` if it has more), else that of the tool call shown with the same id, else the tool call id. Several requests MAY be pending at once, each in its own block.

#### Scenario: A request shows
- **WHEN** the agent asks permission for tool call `Edit a.txt` with options `Yes` (`allow_once`) and `No` (`reject_once`)
- **THEN** `Permission: Edit a.txt\n1. Yes (allow_once)\n2. No (reject_once)` appears above the prompt line and nothing is answered yet

#### Scenario: A request without a title
- **WHEN** the request's tool call has no title and no call with its id is shown
- **THEN** the header line is `Permission: ` followed by the tool call id

### Requirement: Answering a permission request
`C-c C-y` in a session buffer (`agent-allow-once`) SHALL answer the oldest pending request with its first `allow_once` option, and `C-c C-n` (`agent-reject-once`) with its first `reject_once` option, wherever point is. `RET` on an option line of any pending request SHALL answer it with that option; on any other line `RET` sends the prompt as before. The answer SHALL be the `selected` outcome with the option's id, and the request's block SHALL become the single line `Chose <name> (<kind>) for <title>`. With no pending request `C-c C-y` and `C-c C-n` SHALL echo `No permission request is pending`; when the oldest has no option of the kind they SHALL echo `No allow_once option for <title>` (or `reject_once`) and answer nothing.

#### Scenario: Allowing
- **WHEN** a request for `Edit a.txt` is pending and `C-c C-y` is typed in the prompt area
- **THEN** the `Yes` option is selected and the block reads `Chose Yes (allow_once) for Edit a.txt`

#### Scenario: Rejecting
- **WHEN** the same request is pending and `C-c C-n` is typed
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

### Requirement: Cancelling ends pending permission requests
When `C-c C-c` sends `session/cancel`, every pending permission request of the session SHALL be answered with the `cancelled` outcome. When the agent exits, pending requests are dropped unanswered. Either way each block SHALL become the line `Cancelled permission for <title>`. A permission request that arrives after the session ended, or after its session buffer was killed, SHALL be answered `cancelled` and SHALL NOT be shown.

#### Scenario: Cancelling with a pending request
- **WHEN** a request for `Edit a.txt` is pending and `C-c C-c` is typed
- **THEN** it is answered `cancelled`, `session/cancel` is sent and the block reads `Cancelled permission for Edit a.txt`

#### Scenario: A request after the session ended
- **WHEN** the agent has exited and a permission request from it arrives
- **THEN** it is answered `cancelled` and the session buffer is unchanged

#### Scenario: A request after the session buffer was killed
- **WHEN** the session buffer was killed and a permission request from its agent arrives before the agent exits
- **THEN** it is answered `cancelled`

### Requirement: Transcript colours follow the theme
When reloaded settings change the theme, every session buffer's text properties and line decorations SHALL change each colour the old theme gave as `dim`, `code_block`, or the `text.title`, `text.literal` or `text.uri` syntax colour to the new theme's colour for the same name. This SHALL apply to completed blocks, the block being streamed, tool calls, the plan and permission requests alike. A colour the old theme did not define for a name SHALL stay as it was. Reloaded settings that leave the theme unchanged SHALL leave session buffers untouched.

#### Scenario: A completed reply after a theme change
- **GIVEN** a session holds a rendered reply with inline code and a thought
- **WHEN** the user's settings change `code_block` and `dim` and are reloaded
- **THEN** the inline code's background and the thought's colour are the new theme's `code_block` and `dim`

#### Scenario: A rule beside a tool call
- **GIVEN** a session shows a tool call
- **WHEN** the reloaded theme changes `dim`
- **THEN** the tool call's rule decoration is in the new `dim`

### Requirement: Command output shows in its tool call
A `tool_call`, or a call started from an update, SHALL open a terminal for the call when its `_meta.terminal_info` names a `terminal_id` or a content item is a terminal, the first of these naming the terminal's id; a call keeps the terminal it opened when later updates replace its content. Every `tool_call` and `tool_call_update` SHALL append the `data` string of `_meta.terminal_output` and of `_meta.terminal_output_delta`, in that order, to the output of the terminal their `terminal_id` names, whichever call announced it. `_meta.terminal_exit` SHALL record that terminal's exit: `signal <signal>` when `signal` is a string, else `exit <code>` when `exit_code` is a non-zero number, else none. Data or an exit for a terminal no shown call opened SHALL be dropped.

The call's own terminal SHALL show as `Output:` and its output in a fenced code block, with ANSI escape sequences and carriage returns removed, in the place of `rawOutput` in the content, and its `terminal` content item SHALL NOT show as `Terminal <id>`. A terminal without output yet SHALL show nothing. The exit, when recorded, SHALL follow the status in the header, `dim` like it. An update that changes only a terminal's output or exit SHALL re-render that call when the next frame is built, once however many such updates came, and an update with fields SHALL rewrite the call at once.

#### Scenario: Streamed chunks
- **WHEN** a call with a terminal content item `t` and `_meta.terminal_info` for `t` is shown, then updates carry `_meta.terminal_output_delta` `{terminal_id: "t", data: "a"}` and `_meta.terminal_output` `{terminal_id: "t", data: "b\n"}`, and the frame is built
- **THEN** the call's content reads `Output:` and a fenced block holding `ab`, with no `Terminal t` line

#### Scenario: Once per frame
- **WHEN** a call's terminal gets three output-only updates
- **THEN** its block is unchanged until the frame is built, then holds all three chunks

#### Scenario: A failing command
- **WHEN** a call's update carries status `failed` and `_meta.terminal_exit` `{terminal_id: "t", exit_code: 2, signal: null}`
- **THEN** its header reads `<title> (execute, failed, exit 2)`

#### Scenario: A signal
- **WHEN** a terminal's exit carries `exit_code: null` and `signal: "SIGTERM"`
- **THEN** the header ends `(execute, failed, signal SIGTERM)`

#### Scenario: A clean exit
- **WHEN** a terminal's exit carries `exit_code: 0`
- **THEN** the header reads `(execute, completed)`

#### Scenario: ANSI escapes
- **WHEN** a chunk is `\u001b[31mred\u001b[0m\r\n`
- **THEN** the output shows `red`

#### Scenario: An unannounced terminal
- **WHEN** an update carries output for a terminal id no shown call opened
- **THEN** the transcript is unchanged

#### Scenario: Streaming through the fake agent
- **WHEN** the fake agent's turn sends a terminal tool call, output chunks under both keys and a failing exit
- **THEN** after the turn the session buffer's call reads `(execute, failed, exit 1)` and its output holds every chunk in order
