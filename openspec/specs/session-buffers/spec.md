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

### Requirement: Cancelling a turn
`C-c C-c` in a session buffer (`agent-cancel`) SHALL send `session/cancel` for the session while a turn runs; the turn ends when the agent answers the prompt, normally with `cancelled`. With no turn running it SHALL echo `No turn is running` and send nothing.

#### Scenario: Cancelling
- **WHEN** a turn runs, `C-c C-c` is typed and the agent ends the turn with `cancelled`
- **THEN** `session/cancel` is sent and `Turn ended: cancelled` appears above the prompt line

### Requirement: Messages render as markdown
Each agent message block, thought block and echoed prompt's text SHALL be rendered as markdown with text properties while its markup stays in the text: ATX and setext headings bold in the `text.title` syntax face, emphasis italic, strong emphasis bold, inline code in the `session-code` face, and link text and autolinks underlined in the `link` face. Heading markers, emphasis delimiters, code span backticks and a link's brackets and destination SHALL be invisible with no placeholder. Every line of a fenced or indented code block SHALL carry the `session-block` background decoration, and a code fence's text SHALL be invisible, its newline kept. A thought block SHALL be in the `session-thought` face wherever no other colour applies. `Turn ended` and `Error` lines SHALL stay plain.

#### Scenario: Hidden markup
- **WHEN** the agent streams `# Title\n\nSome **bold**, *it*, `code` and [docs](https://x.dev).` and a frame is built
- **THEN** the lines read `Title`, an empty line, and `Some bold, it, code and docs.`, with `Title` bold, `bold` bold, `it` italic, `code` and `docs` coloured

#### Scenario: A code block
- **WHEN** the agent streams "```rust\nfn main() {}\n```" and a frame is built
- **THEN** the three lines carry the code block background and read empty, `fn main() {}` and empty

#### Scenario: A dim thought
- **WHEN** the agent streams the thought chunk `thinking` and a frame is built
- **THEN** `thinking` is drawn in the `session-thought` face

#### Scenario: The echoed prompt
- **WHEN** `**hi** RET` is sent
- **THEN** the echo reads `> hi` with `hi` bold

### Requirement: Markup stays in the text
Rendering SHALL NOT change the buffer's text outside tables: search SHALL match hidden markup and copying or killing a rendered message SHALL yield its markdown. A table SHALL be the exception: search, copy and kill SHALL see its drawn text, and its markdown SHALL NOT be in the buffer.

#### Scenario: Swiper finds hidden markup
- **WHEN** a reply `**bold**` is rendered and swiper searches for `**b`
- **THEN** it matches the reply's line

#### Scenario: Copy yields markdown
- **WHEN** a rendered reply `**bold**` is marked whole and `M-w` is typed
- **THEN** the kill ring's top is `**bold**`

#### Scenario: Copy yields a drawn table
- **WHEN** the table `| a | b |\n|---|--:|\n| x | 10 |` is rendered, its two lines are marked whole and `M-w` is typed
- **THEN** the kill ring's top is ` a   b \n x  10 \n`

#### Scenario: Search sees a drawn table
- **WHEN** that table is rendered and swiper searches for `| x`, then for `x  10`
- **THEN** the first search matches nothing and the second matches the table's second line

### Requirement: Streamed messages re-render once per frame
A streamed chunk's text SHALL be inserted at once. If chunks have arrived since the block's previous rendering, a frame that shows the session buffer SHALL re-render that block once, regardless of the number of chunks. A session is shown when any pane in the active workspace displays it, including an inactive pane or the origin pane underneath an open minibuffer. Frames that do not show the session SHALL defer its streamed-message rendering while retaining the pending text and formatting work. The first frame that shows it again SHALL include formatting for all accumulated chunks, even if no further chunk arrives. Only the block being streamed SHALL be re-rendered; earlier blocks SHALL keep their properties. A block that stops streaming SHALL be rendered before the next block starts, including while the session is hidden.

#### Scenario: Chunks wait for the frame
- **GIVEN** a shown session buffer
- **WHEN** the chunks `**a` and `b**` arrive and no frame is built yet
- **THEN** the text reads `**ab**` and the block has no properties yet; after a frame is built `ab` is bold and the `**` hidden

#### Scenario: Earlier blocks are untouched
- **GIVEN** a reply was rendered
- **WHEN** another reply streams after the next prompt and frames are built
- **THEN** the first reply's properties remain as they were

#### Scenario: Hidden streaming text catches up on display
- **GIVEN** a session is hidden behind another buffer or belongs to an inactive workspace
- **WHEN** chunks `**a` and `b**` arrive while frames show other buffers
- **THEN** its stored text reads `**ab**`, rendering remains pending, and the first frame showing that session displays `ab` in bold with the `**` hidden without needing another chunk

#### Scenario: An inactive pane is still shown
- **GIVEN** a session buffer is displayed in an inactive split pane of the active workspace
- **WHEN** streamed chunks arrive and the next frame is built
- **THEN** that pane shows their updated formatting

#### Scenario: A prompt leaves its origin pane shown
- **GIVEN** a minibuffer prompt is active above its session origin pane
- **WHEN** the session receives streamed chunks and the next frame is built
- **THEN** the origin pane shows their updated formatting

#### Scenario: A hidden block finishes before another begins
- **GIVEN** a hidden session has an unrendered message block
- **WHEN** a new message kind, tool call, plan, permission prompt or submitted prompt ends that block
- **THEN** the completed block receives its final formatting before the next block begins and retains that formatting when later shown

#### Scenario: A hidden session is killed
- **GIVEN** a hidden session has pending streamed-message formatting
- **WHEN** its buffer is killed and another frame is built
- **THEN** remaining buffers render normally and the killed session's pending work is discarded

### Requirement: Tool calls show folded
Each `tool_call` update SHALL add a block above the prompt line, after any block being streamed, separated from the next by a blank line. Its first line, the header, SHALL read `<fold> <status> <icon> <verb> <target>`: `▸` when folded or `▾` when open, in the `session-thought` face; `✓` for `completed` in `status-running`, `✗` for `failed` in `status-failed` or `●` otherwise in `status-running`; the kind's icon, drawn over two spaces in `session-thought`: book for `read`, magnifier for `search`, pencil for `edit`, `delete` and `move`, terminal for `execute`, globe for `fetch`, wrench otherwise; the kind's verb in `session-thought`, in the past tense once `completed` or `failed` (`Read`, `Searched`, `Edited`, `Deleted`, `Moved`, `Ran`, `Fetched`, `Used`) and the present before that (`Reading`, `Searching`, `Editing`, `Deleting`, `Moving`, `Running`, `Fetching`, `Using`); and the target, in `session-code` when it is a command and as plain text otherwise. A rule in the `session-thought` face SHALL be beside it. The target SHALL be the first of those that applies: a command's `rawInput.command` for an `execute` call or one with a terminal; `rawInput.url` for `fetch`; for `read`, `edit`, `delete` and `move`, the first location's path, else the first diff's, relative to the project's primary root when under it; else the title. A target SHALL be its first line, followed by ` …` when it has more, a title without surrounding backticks. A call with diff items SHALL show ` +<added> −<removed>` after its target, the counts of added and removed lines over all its diffs, `+<added>` in `status-running` and `−<removed>` in `status-failed`, as the status marks. When the call has content, the header SHALL be followed by a newline and the content, which SHALL start folded: invisible, with no placeholder.

The content SHALL hold, in order and each separated by a blank line: `Input:` and the call's whole `rawInput` as pretty-printed JSON in a fenced `json` code block, when it has one, it is not a command with `rawInput.command` and it has no diff item; each content item, a text block as its markdown, a resource link as its URI, a diff as its unified diff's hunks from the first `@@` line, without the path or the `---`/`+++` lines, in a fenced `diff` code block with added lines in `diff-added`, removed lines in `diff-removed` and `@@` lines in `diff-hunk`, a terminal other than the call's own terminal (see Command output shows in its tool call) as `Terminal <id>`, and any other item as `[<type>]`; and the output in a fenced code block: `$ <command>` above it for a command with `rawInput.command`, else `Output:` before the fence, holding the call's terminal output when it has a terminal, else the call's whole `rawOutput` when it has one and no content item, or, for a command, whatever its text items: a string as itself, a list of text blocks as their text joined, anything else as pretty-printed JSON. A command without output SHALL show `$ <command>` alone. Only a command's output is cut, when shown (see A command's open body is cut at 20 lines). A fence SHALL be longer than any run of backticks in what it encloses.

When open, a fence line SHALL take no row: the fence and the newline that joins it to the block SHALL be invisible, with no placeholder, and stay in the buffer's text. Every line of open content SHALL be indented two columns past where its header's status mark is drawn, its `session-block` background starting there and ending at the window's right text inset.

#### Scenario: A tool call starts folded
- **WHEN** the agent sends a `tool_call` titled `` `ls` `` of kind `execute`, status `pending` and `rawInput` `{"command":"ls"}`
- **THEN** a block reads `▸ ●`, the terminal icon, `Running ls`, `ls` in `session-code`, then its content, of which nothing shows

#### Scenario: A finished read
- **WHEN** a `read` call with the location `<primary>/src/a.rs` completes
- **THEN** its header reads `✓`, the book icon, `Read src/a.rs`

#### Scenario: A failed call
- **WHEN** a call's status becomes `failed`
- **THEN** its header shows `✗` in `status-failed`

#### Scenario: A title as the target
- **WHEN** a call of kind `other` is titled `ToolSearch`
- **THEN** its header shows the wrench icon and `Using ToolSearch`

#### Scenario: A call without content
- **WHEN** the agent sends a `tool_call` with no `rawInput`, content or `rawOutput`
- **THEN** its block is the header line alone

#### Scenario: A multi-line title
- **WHEN** a call's title is `Run\nmore`
- **THEN** its header's target reads `Run …`

#### Scenario: Full output
- **WHEN** a `read` call's `rawOutput` is a 50-line string and it has no content items, and its header is toggled open
- **THEN** all 50 lines show under `Output:`

#### Scenario: An edit shows its diff
- **WHEN** an `edit` call with a diff item and `rawInput` is toggled open
- **THEN** its first shown body line starts `@@`, added and removed lines show in their faces, and no path, `---`, `+++` or `Input:` line shows

#### Scenario: An edit counts its lines
- **WHEN** an `edit` call's diff replaces one line with six
- **THEN** its header ends ` +6 −1`, `+6` in `status-running` and `−1` in `status-failed`

#### Scenario: Fences take no row
- **WHEN** a command with two output lines is opened
- **THEN** the rows under its header are `$ <command>` and the two lines, and the buffer's text still holds both fences

#### Scenario: An open body is indented
- **WHEN** a call in a group is opened
- **THEN** its body's lines are drawn further right than its header's status mark, on a `session-block` background that stops at the window's right inset

### Requirement: Tool calls update in place
A `tool_call_update` SHALL merge its fields into the call with that id and rewrite the call's block where it is, even with later blocks after it, keeping whether it is folded; the rest of the transcript and the prompt area SHALL be unchanged. A `tool_call` with an id already shown SHALL replace that call the same way. An update for an unknown id SHALL start a block when it carries a title and be kept unrendered otherwise.

#### Scenario: Status updates
- **WHEN** a call is shown, the agent streams a message after it, then sends updates to `in_progress` and then `completed` with output
- **THEN** the call's header reads `✓`, `Ran` above the message, and its content holds the output

#### Scenario: A repeated tool_call
- **WHEN** an unfolded call is sent again as a `tool_call` with the same id and status `completed`
- **THEN** its one block's header reads `✓`, `Ran` and stays unfolded

#### Scenario: Point on the header
- **WHEN** point is on a call's header and an update changes its status
- **THEN** point is still on the header line

#### Scenario: An unknown id
- **WHEN** a `tool_call_update` arrives for an id never shown, without a title
- **THEN** the transcript is unchanged

### Requirement: TAB toggles a tool call
Outside a pasted range, `TAB` in a session buffer (`agent-toggle-tool-call`) with point on a tool call's header line SHALL show its content if folded and hide it otherwise. With point anywhere in an unfolded call's content it SHALL hide the content and move point to the start of the header. The header's fold marker SHALL follow. Shown content SHALL be rendered as markdown, as messages are, with the rule beside each of its lines. A thought row SHALL toggle the same way. On a group line `TAB` SHALL open or fold the group. Inside a group, on a row with nothing to open, on an answered line or on text the fold hides, it SHALL toggle the group, moving point to the group line when it leaves it hidden. With point anywhere else it SHALL echo `No tool call here`.

#### Scenario: Unfolding and folding
- **WHEN** point is on a folded call's header and `TAB` is typed, then `TAB` again
- **THEN** the content shows under the header with the rule beside it and its fences hidden, then hides again, the fold marker going from `▸` to `▾` and back

#### Scenario: Folding from inside the content
- **WHEN** a call is unfolded, point is on a line of its content and `TAB` is typed
- **THEN** the content hides and point is at the start of the header

#### Scenario: Not on a header
- **WHEN** `TAB` is typed in the prompt area outside any pasted range
- **THEN** `No tool call here` is echoed and nothing changes

#### Scenario: A call without content
- **WHEN** `TAB` is typed on the header of a call without content and an update then brings output
- **THEN** the output arrives folded

#### Scenario: Unfolded content stays open
- **WHEN** a call is unfolded and an update brings new output
- **THEN** the call stays unfolded, showing the new output

#### Scenario: Opening a group, then a row
- **WHEN** `TAB` is typed on a folded group line, then on a row
- **THEN** the rows show, then that call's content shows, and the group line's marker reads `▾`

#### Scenario: Folding back level by level
- **WHEN** point is in an open row's content in an open group and `TAB` is typed, then `TAB` on the group line
- **THEN** the call folds with point on its row, then the group folds

#### Scenario: A row with nothing to open
- **WHEN** `TAB` is typed on a row without content in an open group
- **THEN** the group folds and point is on its line

### Requirement: The plan shows and is replaced in place
The first `plan` update of a session SHALL add a block above the prompt line, after any block being streamed, separated from the next by a blank line. The block SHALL read `Plan` in `session-thought`, then one line per entry in order: two spaces, a mark and a space, then the entry's content's first line, followed by ` …` when it has more lines. The mark SHALL be `✓` for `completed` and `○` for `pending`, each with its line's content in `session-thought`, and `●` in `status-waiting` for `in_progress`, its content plain. The priority SHALL NOT show. The block SHALL have no rule beside it. Each later `plan` update SHALL replace the block's entries where the block is, even with later blocks after it; the rest of the transcript and the prompt area SHALL be unchanged. A plan with no entries SHALL show the `Plan` line alone.

#### Scenario: A plan shows
- **WHEN** the agent sends a plan with `Read the code` (`in_progress`, `high`) and `Write tests` (`pending`, `medium`)
- **THEN** a block reads `Plan`, `  ● Read the code`, `  ○ Write tests` above the prompt line, `●` in `status-waiting`

#### Scenario: A new plan replaces the old one in place
- **WHEN** a plan is shown, the agent streams a message after it, then sends a plan with `Read the code` `completed` and a third entry `Ship` (`pending`, `low`)
- **THEN** there is one plan block, above the message, reading `Plan`, `  ✓ Read the code`, `  ○ Write tests`, `  ○ Ship`

#### Scenario: An empty plan
- **WHEN** the agent sends a plan with no entries after a plan with entries
- **THEN** the plan block reads `Plan` alone

### Requirement: The mode line shows the current mode
A session buffer's mode line SHALL show the first config option of category `mode`, when it is a select, as `[<current value>]` on the left, following the model and effort and before the session title. It SHALL follow `session/new` or `session/load`, `config_option_update` and `session/set_config_option` answers, and omit the mode when no mode select exists or the session is not ready. The mode SHALL use `status-info` for `plan` and `read-only`, plain `mode-line` for `default`, `status-waiting` for `acceptEdits` and `workspace-write`, `status-running` for `auto` and `agent`, and `status-failed` for `bypassPermissions` and `agent-full-access`. Unknown modes SHALL use plain `mode-line`.

#### Scenario: The mode at start
- **GIVEN** a starting session
- **WHEN** it becomes ready with current mode `auto`
- **THEN** its mode line shows `[auto]` in `status-running`

#### Scenario: A mode change
- **GIVEN** a session in `auto`
- **WHEN** a config update or S-TAB's answer reports `plan`
- **THEN** its mode segment becomes `[plan]` in `status-info`

#### Scenario: Not ready
- **GIVEN** a session not yet ready or whose latest config has no mode select
- **WHEN** its mode line is displayed
- **THEN** the modeline omits the mode segment

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
A `session/request_permission` from a session whose `session/new` was answered and whose agent hasn't exited SHALL add a block above the prompt line and stay pending: the line `Permission: <title>`, then a line per option in order, `<n>. <name> (<kind>)`, numbered from 1, with the ACP name of the kind in the `session-thought` colour and a `session-thought` rule beside every line of the block. The title is the first line of the request's tool call title (followed by ` …` if it has more), else that of the tool call shown with the same id, else the tool call id. Several requests MAY be pending at once, each in its own block.

#### Scenario: A request shows
- **WHEN** the agent asks permission for tool call `Edit a.txt` with options `Yes` (`allow_once`) and `No` (`reject_once`)
- **THEN** `Permission: Edit a.txt\n1. Yes (allow_once)\n2. No (reject_once)` appears above the prompt line and nothing is answered yet

#### Scenario: A request without a title
- **WHEN** the request's tool call has no title and no call with its id is shown
- **THEN** the header line is `Permission: ` followed by the tool call id

### Requirement: Answering a permission request
`C-c C-y` in a session buffer (`agent-allow-once`) SHALL send the multi-select question whose block point is on, as agent-questions describes; elsewhere it SHALL answer the oldest pending permission request with its first `allow_once` option, passing pending questions by. `C-c C-k` (`agent-reject-once`) SHALL answer the oldest pending request: a permission request with its first `reject_once` option, a question by skipping it, wherever point is. `RET` on an option line of any pending request SHALL answer it with that option; on any other line `RET` sends the prompt as before. The answer SHALL be the `selected` outcome with the option's id, and the request's block SHALL become a single line in `session-thought`, with no rule, that stays in the transcript: `✓ allowed once` for an `allow_once` option, `✓ allowed always` for `allow_always`, `✗ rejected` for `reject_once` and `✗ rejected always` for `reject_always`. With no pending permission request `C-c C-y`, and with no pending request `C-c C-k`, SHALL echo `No permission request is pending`; when the oldest has no option of the kind they SHALL echo `No allow_once option for <title>` (or `reject_once`) and answer nothing.

#### Scenario: Allowing
- **WHEN** a request for `Edit a.txt` is pending and `C-c C-y` is typed in the prompt area
- **THEN** the `Yes` option is selected and the block reads `✓ allowed once`

#### Scenario: Rejecting
- **WHEN** the same request is pending and `C-c C-k` is typed
- **THEN** the `No` option is selected and the block reads `✗ rejected`

#### Scenario: Allowing always
- **WHEN** point is on the line of an `allow_always` option and `RET` is typed
- **THEN** that option is selected and the block reads `✓ allowed always`

#### Scenario: Rejecting always
- **WHEN** point is on the line of a `reject_always` option and `RET` is typed
- **THEN** that option is selected and the block reads `✗ rejected always`

#### Scenario: RET on an option
- **WHEN** point is on the line `2. No (reject_once)` and `RET` is typed
- **THEN** the `No` option is selected and no prompt is sent

#### Scenario: Oldest first
- **WHEN** requests for `A` and then `B` are pending and `C-c C-y` is typed twice
- **THEN** `A` is answered first, then `B`

#### Scenario: Nothing to answer
- **WHEN** no request is pending, or the oldest has no `allow_once` option, and `C-c C-y` is typed
- **THEN** `No permission request is pending`, or `No allow_once option for <title>`, is echoed and nothing is answered

#### Scenario: Allowing passes a question by
- **WHEN** a question and then a permission request for `A` are pending and `C-c C-y` is typed
- **THEN** `A` is answered and the question stays pending

#### Scenario: Sending the multi-select question at point
- **WHEN** a permission request for `A` and then a multi-select question are pending, point is on the question's lines and `C-c C-y` is typed
- **THEN** the question is sent and `A` stays pending

### Requirement: Cancelling ends pending permission requests
When `C-c C-c` sends `session/cancel`, the agent's answer to the prompt ends a turn, or the session buffer is killed, every pending permission request of the session SHALL be answered with the `cancelled` outcome. When the agent exits, pending requests are dropped unanswered. Each way, each block SHALL become the line `✗ cancelled` in `session-thought`, with no rule. A permission request that arrives after the session ended, or after its session buffer was killed, SHALL be answered `cancelled` and SHALL NOT be shown.

#### Scenario: Cancelling with a pending request
- **WHEN** a request for `Edit a.txt` is pending and `C-c C-c` is typed
- **THEN** it is answered `cancelled`, `session/cancel` is sent and the block reads `✗ cancelled`

#### Scenario: The turn ends with a pending request
- **WHEN** a request for `Edit a.txt` is pending and the agent answers the prompt, with any stop reason or an error
- **THEN** it is answered `cancelled` and the block reads `✗ cancelled`

#### Scenario: A request after the session ended
- **WHEN** the agent has exited and a permission request from it arrives
- **THEN** it is answered `cancelled` and the session buffer is unchanged

#### Scenario: A request after the session buffer was killed
- **WHEN** the session buffer was killed and a permission request from its agent arrives before the agent exits
- **THEN** it is answered `cancelled`

#### Scenario: Killing the buffer with a pending request
- **WHEN** a request for `Edit a.txt` is pending and the session buffer is killed
- **THEN** it is answered `cancelled` before the agent is stopped

### Requirement: Command output shows in its tool call
A `tool_call`, or a call started from an update, SHALL open a terminal for the call when its `_meta.terminal_info` names a `terminal_id` or a content item is a terminal, the first of these naming the terminal's id; a call keeps the terminal it opened when later updates replace its content. Every `tool_call` and `tool_call_update` SHALL append the `data` string of `_meta.terminal_output` and of `_meta.terminal_output_delta`, in that order, to the output of the terminal their `terminal_id` names, whichever call announced it. `_meta.terminal_exit` SHALL record that terminal's exit: `signal <signal>` when `signal` is a string, else `exit <code>` when `exit_code` is a non-zero number, else none. Data or an exit for a terminal no shown call opened SHALL be dropped.

The call's own terminal SHALL show its output in the fenced code block where `rawOutput` would go, with ANSI escape sequences and carriage returns removed and its `terminal` content item SHALL NOT show as `Terminal <id>`. A terminal without output yet SHALL show nothing. The exit, when recorded, SHALL show at the header's right in `status-failed`, in place of the duration. An update that changes only a terminal's output or exit SHALL re-render that call when the next frame showing its session is built, once however many such updates came, and an update with fields SHALL rewrite the call at once. Frames that do not show the session SHALL defer that rendering while retaining the output and exit. A session is shown as for streamed messages.

The SGR escapes removed from the terminal output and from text content items SHALL become faces on the chars they precede, shown when the call is open: foreground (`30`–`37`, `90`–`97`, `38;5;<n>` for n below 16) and background (`40`–`47`, `100`–`107`, `48;5;<n>`) in the theme's ANSI colours, bold (`1`), underline (`4`), and their resets (`0`, `39`, `49`, `22`, `24`). Other colours SHALL show in the default colour, and other attributes and escapes SHALL be ignored. The buffer's text SHALL hold no escapes, so copying the output yields plain text.

#### Scenario: Streamed chunks
- **WHEN** a call with a terminal content item `t` and `_meta.terminal_info` for `t` is shown, then updates carry `_meta.terminal_output_delta` `{terminal_id: "t", data: "a"}` and `_meta.terminal_output` `{terminal_id: "t", data: "b\n"}`, and the frame is built
- **THEN** the call's content reads a fenced block holding `ab`, with no `Terminal t` line

#### Scenario: Once per frame
- **WHEN** a call's terminal gets three output-only updates
- **THEN** its block is unchanged until the frame is built, then holds all three chunks

#### Scenario: A failing command
- **WHEN** a call's update carries status `failed` and `_meta.terminal_exit` `{terminal_id: "t", exit_code: 2, signal: null}`
- **THEN** its header shows `✗` and ends `exit 2`

#### Scenario: A signal
- **WHEN** a terminal's exit carries `exit_code: null` and `signal: "SIGTERM"`
- **THEN** the header ends `signal SIGTERM`

#### Scenario: A clean exit
- **WHEN** a terminal's exit carries `exit_code: 0`
- **THEN** the header shows `✓` and its duration

#### Scenario: ANSI escapes
- **WHEN** a chunk is `\u001b[31mred\u001b[0m\r\n`
- **THEN** the output shows `red`, in the theme's ANSI colour 1 when the call is open

#### Scenario: Bold, underline and reset
- **WHEN** the output is `\u001b[1;4;92mok\u001b[22mx\u001b[0my`
- **THEN** `ok` is bold, underlined and in ANSI colour 10, `x` underlined in colour 10 but not bold, and `y` has no face

#### Scenario: Colours beyond the palette
- **WHEN** the output is `\u001b[38;5;196ma\u001b[38;2;1;2;3mb\u001b[38;5;4mc`
- **THEN** `a` and `b` have no face and `c` is in ANSI colour 4

#### Scenario: Codex's coloured text content
- **WHEN** a call's text content item is `\u001b[34mfile\u001b[0m`
- **THEN** it reads `file`, in ANSI colour 4 when the call is open

#### Scenario: An unannounced terminal
- **WHEN** an update carries output for a terminal id no shown call opened
- **THEN** the transcript is unchanged

#### Scenario: Streaming through the fake agent
- **WHEN** the fake agent's turn sends a terminal tool call, output chunks under both keys and a failing exit
- **THEN** after the turn the session buffer's call shows `✗` and `exit 1`, and its output holds every chunk in order

#### Scenario: Hidden command output catches up
- **GIVEN** a session is hidden behind another buffer or belongs to an inactive workspace
- **WHEN** output-only updates arrive for its call while frames show other buffers
- **THEN** its block is unchanged until the first frame showing the session, which holds every chunk in order without needing another update

#### Scenario: Open and folded calls agree
- **GIVEN** two calls receive the same chunks, one open and one folded
- **WHEN** the frame is built and the folded call is opened
- **THEN** both blocks hold the same text and faces

### Requirement: The session buffer is named after the session's title
A `session_info_update` whose `title` is a string SHALL rename the session buffer to `*<agent>: <title>*<<workspace>>`, made unique, where `<title>` is the title's first line with surrounding whitespace trimmed, cut to its first 40 chars with `…` appended when longer. A title that is empty after trimming, or a `title` that is null, SHALL rename the buffer back to `*<agent>*<<workspace>>`, made unique. An update without `title` SHALL leave the name alone. `<agent>` and `<workspace>` SHALL be the names the buffer was started with. Every titled update SHALL rename again; an update after the session has ended SHALL be ignored.

#### Scenario: The agent titles the session
- **WHEN** the session started as `*claude*<main>` receives a `session_info_update` with title `Fruit list`
- **THEN** the buffer is named `*claude: Fruit list*<main>`

#### Scenario: A later title renames again
- **WHEN** a session titled `Reply with a list` receives a `session_info_update` with title `Two-fruit markdown list`
- **THEN** the buffer is named `*codex: Two-fruit markdown list*<main>`

#### Scenario: A long or multi-line title
- **WHEN** the title is a 60-char first line followed by a second line
- **THEN** the name holds the first line's first 40 chars followed by `…`, and no text from the second line

#### Scenario: An update without a title
- **WHEN** a `session_info_update` carries only `_meta`
- **THEN** the buffer keeps its name

#### Scenario: Clearing the title
- **WHEN** a session named `*claude: Fruit list*<main>` receives a `session_info_update` with title null
- **THEN** the buffer is named `*claude*<main>`

#### Scenario: Two sessions with one title
- **WHEN** two claude sessions in `main` both receive the title `Fruit list`
- **THEN** one buffer is named `*claude: Fruit list*<main>` and the other `*claude: Fruit list*<main><2>`

### Requirement: Session buffer names belong to the agent
A session buffer's name SHALL change only by its session's title. Renaming the workspace SHALL NOT rename it, and any command that renames buffers by hand SHALL refuse a session buffer.

#### Scenario: Renaming the workspace
- **WHEN** workspace `main` holding `*claude: Fruit list*<main>` is renamed to `work`
- **THEN** the session buffer is still named `*claude: Fruit list*<main>`

### Requirement: Loading a session into a new session buffer
Loading a listed session SHALL create a session buffer as `agent-start` does, owned by the current workspace, named after the listed title as a `session_info_update` would name it, and a stderr log buffer, without showing the session buffer; the agent SHALL be spawned to load that session in the project's primary repo, waiting for the environment capture as a start does. The echo area SHALL show `Loading <agent> session...`, and `<agent> session ready (modes: <values>)` once the load answers. When the load answers, the selected window SHALL show the session buffer if it still shows the buffer the load was started from; otherwise nothing is switched. Before the load answers, `RET` in the session buffer SHALL echo `<agent> is not ready`.

#### Scenario: Loading shows the history
- **WHEN** a `claude` session titled `Fix it` is loaded in workspace `work` and the agent replays a user message `hi` and an agent message `hello`
- **THEN** `*claude: Fix it*<work>` exists, unshown, and its text is `> hi`, a blank line, `hello`, a blank line and the prompt line

#### Scenario: The loaded session shows once ready
- **WHEN** the load answers while the selected window still shows the sessions buffer it was started from
- **THEN** the selected window shows the session buffer

#### Scenario: A load answered elsewhere stays hidden
- **WHEN** the load answers after the selected window was switched to another buffer
- **THEN** the selected window keeps that buffer

### Requirement: Replayed user messages show as prompts
A `user_message_chunk` with text SHALL be put above the prompt line as its own block, `> ` followed by the text and a blank line, rendered as markdown as a sent prompt is, ending any streamed block; chunks with other content SHALL show nothing.

#### Scenario: A user message between agent messages
- **WHEN** an agent message chunk `a`, a user message chunk `b` and an agent message chunk `c` arrive
- **THEN** the transcript reads `a`, `> b` and `c`, each followed by a blank line

### Requirement: Transcripts are drawn in the current theme
Session buffers SHALL store face names, not colours, and the display SHALL resolve them against the current theme at each redisplay. A theme change SHALL apply to completed blocks, the block being streamed, tool calls and their ANSI colours, the plan and permission requests alike, and two faces sharing a colour in the old theme SHALL keep their own colours in the new one.

#### Scenario: A completed reply after a theme change
- **GIVEN** a session holds a rendered reply with a code block and a thought
- **WHEN** `theme` is changed and the settings are reloaded
- **THEN** the code block's background and the thought's colour are the new theme's `session-block` and `session-thought`

#### Scenario: Faces sharing a colour
- **GIVEN** the old theme gives `session-thought` and `session-block` the same colours
- **WHEN** the new theme gives them different ones
- **THEN** each takes its own

#### Scenario: A rule beside a tool call
- **GIVEN** a session shows a tool call
- **WHEN** the reloaded theme changes `session-thought`
- **THEN** the tool call's rule decoration is in the new `fg`

#### Scenario: ANSI output
- **GIVEN** an open tool call shows output in ANSI red
- **WHEN** the reloaded theme changes `ansi`
- **THEN** the output is drawn in the new ANSI colour 1

### Requirement: Session mode lines identify model effort and title
A session mode line's left group SHALL contain, in order, the model and effort display names joined by ` · ` as one segment, the current mode segment, and the session title in `mode-line-buffer-id`. Model and effort SHALL come from the first select config options in categories `model` and `thought_level`, finding the current value in either flat or grouped options. The current value itself SHALL be the fallback when unlisted. Missing model or effort SHALL be omitted without a joining dot. These fields SHALL follow readiness and each config update or set-config answer. The title SHALL follow the same normalization and 40-character limit as the buffer name and fall back to the agent's registry name when unset. The generic buffer state, decorated buffer name and point position SHALL be replaced by these segments. Inactive styling and width fitting SHALL follow the existing mode-line rules.

In the selected window the model and effort SHALL be coloured by item quality, the ` · ` between them in `mode-line-dim`. The model's quality SHALL come from the family word in its display name or value, matched whole and case-insensitively: `fable` and `astra` `quality-legendary`, `opus` and `sol` `quality-epic`, `sonnet` and `terra` `quality-rare`, `haiku` and `luna` `quality-uncommon`. The effort's quality SHALL come from its value: `default` `quality-poor`, `low` `quality-common`, `medium` `quality-uncommon`, `high` `quality-rare`, `xhigh` `quality-epic`, `max` `quality-legendary`, `ultra` `quality-artifact`. A model or effort matching none SHALL stay in `mode-line-dim`.

#### Scenario: Ready configuration and title
- **GIVEN** a session titled `Fix it` with model `opus` named `Opus 5.5` and effort `high` named `High`
- **WHEN** its mode is `plan`
- **THEN** the left group reads `Opus 5.5 · High │ [plan] │ Fix it`

#### Scenario: Changed or grouped configuration
- **GIVEN** a ready session
- **WHEN** updated model or thought-level config has its current value in a group
- **THEN** the model line uses that value's display name

#### Scenario: Missing or unknown fields
- **GIVEN** a session with no title, model or effort
- **WHEN** its mode line is displayed
- **THEN** the title segment shows the agent name and unavailable fields are omitted

#### Scenario: Quality colours
- **GIVEN** the selected window shows a session
- **WHEN** its model and effort are `Opus 5.5` and `high`, then `6 Astra` and `ultra`, then `6 Luna` and `low`, then `Default (recommended)` and `default`
- **THEN** they are in `quality-epic` and `quality-rare`, then `quality-legendary` and `quality-artifact`, then `quality-uncommon` and `quality-common`, then `mode-line-dim` and `quality-poor`, with ` · ` in `mode-line-dim`

#### Scenario: Older models keep their family's colour
- **WHEN** the model is `Opus 4.8` or `5.6 Sol`
- **THEN** it is in `quality-epic`

#### Scenario: Inactive windows drop quality colours
- **WHEN** the session's window isn't selected
- **THEN** the model and effort are in `mode-line-inactive`

### Requirement: Session mode lines show lifecycle status
The right group SHALL show `● working` in `status-running` while a prompt awaits its answer, `● awaiting permission` in `status-waiting` while any permission request is pending, `● awaiting answer` in `status-waiting` while a question is pending and no permission request is, or `● exited` in `status-failed` after exit or failed startup, with exited taking precedence over awaiting permission, which takes precedence over awaiting answer, which takes precedence over working. A live session without a running prompt or pending request SHALL omit status. Answering one of several requests SHALL keep awaiting-permission status until all are answered.

#### Scenario: Turn and permission lifecycle
- **GIVEN** a live idle session
- **WHEN** a prompt starts, two permissions arrive, one is answered, then the other is answered, then the prompt ends
- **THEN** the status progresses through working, awaiting permission, awaiting permission, working, then no status

#### Scenario: Session ends
- **GIVEN** a running session with a pending request or a starting session
- **WHEN** the agent exits or fails to start
- **THEN** its status is `● exited`

### Requirement: Session mode lines show context usage
The core SHALL retain the latest `usage_update`'s `used` and `size` and show `ctx N%` after status, where N is the integer floor of `used * 100 / size`, capped at 100. The whole segment SHALL be in `status-running` when N is under 50, `status-waiting` from 50 to 79 and `status-failed` from 80. Before usage arrives, or when the latest size is zero, the context segment SHALL be omitted. Updates after exit SHALL be ignored. No cost or rate-limit segment SHALL be shown.

#### Scenario: Recorded usage
- **GIVEN** each recorded adapter session
- **WHEN** its recorded usage updates are replayed
- **THEN** each mode line's context percentage follows the latest used and size values

#### Scenario: Invalid or excessive usage
- **GIVEN** a session receiving usage
- **WHEN** size is zero, then used is greater than a positive size
- **THEN** context is first omitted and then shown as `ctx 100%`

#### Scenario: Context colour follows how full it is
- **WHEN** usage is 49%, 50%, 79% and 80% of the size in turn
- **THEN** `ctx N%` is in `status-running`, `status-waiting`, `status-waiting` and `status-failed`

### Requirement: Folded output layout skips hidden lines
Laying out a folded tool call SHALL jump across its hidden lines rather than repeatedly expanding the viewport through them, so a fixed appended chunk does not make folded redisplay grow with retained output.

#### Scenario: A large folded output before visible text
- **GIVEN** a folded call hides many lines and a visible line follows it
- **WHEN** a frame showing the call is built
- **THEN** the folded header and following line are shown with their faces, without walking the hidden lines in viewport-sized batches

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

### Requirement: Messages show their clock time
A user or agent message that arrives live SHALL show the local `HH:MM:SS` of its arrival right-aligned on its first line, in the `annotation` face. An agent message SHALL show it when its block ends; a sent prompt SHALL show it at once.

#### Scenario: A sent prompt shows its time
- **WHEN** the time of day is 14:01:05 and `hello RET` is typed
- **THEN** the echo's first line ends with `14:01:05` in `annotation`, right-aligned

#### Scenario: A reply shows the time its first chunk arrived
- **WHEN** a message's first chunk arrives at 14:03:22, the clock passes 14:04 and the turn ends
- **THEN** the message's first line ends with `14:03:22`

#### Scenario: Seconds count between minute updates
- **WHEN** the frontend last gave the time as 14:01:00 and a prompt is sent 9.6s later
- **THEN** the echo's first line ends with `14:01:09`

### Requirement: Thoughts show how long they took
A live thought's block SHALL, when it ends, start with the line `Thought for <n>s`, the whole seconds from its first chunk to its end, in the thought's face.

#### Scenario: A thought's header
- **WHEN** a thought's first chunk arrives and 4.2s later a message chunk arrives
- **THEN** the thought's block starts with `Thought for 4s`

### Requirement: Tool calls show how long they ran
When a live tool call first reaches `completed` or `failed`, its header SHALL show its duration right-aligned in `annotation`: the time since it arrived less the time it waited for permission, as `0.3s` under a minute and `2m 05s` from a minute on. A command that recorded an exit SHALL show that instead, and a permission wait SHALL show over both.

#### Scenario: A finished call shows its duration
- **WHEN** a tool call arrives and completes 0.3s later
- **THEN** its header ends with `0.3s`

#### Scenario: A permission wait is not run time
- **WHEN** a tool call arrives, waits 10s for permission and completes 2s after the answer
- **THEN** its header ends with `2.0s`

### Requirement: A tool call counts its permission wait
While a permission request names a shown tool call, that call's header SHALL show `waiting <m>:<ss>` right-aligned in `annotation`, counted from the oldest such request, ticking each second, and SHALL drop it when answered or cancelled. A request naming no shown call shows no counter.

#### Scenario: The counter ticks
- **WHEN** a permission request for a shown call has been open for 12s and the session is shown
- **THEN** the call's header ends with `waiting 0:12`

#### Scenario: Answering ends the counter
- **WHEN** the request is answered
- **THEN** the call's header no longer shows `waiting`

### Requirement: The working line
While a turn runs, the last block above the prompt line SHALL be the line `Working for <duration>` in `session-thought`, counted from sending the prompt, permission waits included, ticking each second; `<duration>` reads `12s`, `2m 05s` or `1h 02m 05s`. Output SHALL go in above it. When the turn ends, by the answer to `session/prompt`, a cancel or the agent exiting, the same line SHALL become `Worked <duration>` with the clock time right-aligned, both in `annotation`, and stay where it is. The settled line SHALL be drawn as a rule: indented two columns, with a 1 px line in the `separator` foreground through the middle of the row from the window's left edge to a column before the label, from a column after the label to a column before the time, and from a column after the time to the window's right edge.

#### Scenario: It ticks above the prompt
- **WHEN** a turn has run for 136s and the session is shown
- **THEN** the line above the prompt line reads `Working for 2m 16s` with no rule, and output arriving goes in above it

#### Scenario: The line settles however the turn ends
- **WHEN** a turn ends with `end_turn`, with `cancelled`, with an error answer or by the agent exiting
- **THEN** the line reads `Worked <duration>` and its time in the same place, under any `Turn ended` or `Error` line, drawn as a rule

### Requirement: Only shown sessions tick
The core SHALL wake to tick only while a shown session's turn runs or its permission request is open. A hidden session's working line and counters SHALL be current as soon as it is shown; ending a hidden session's turn SHALL settle its line at once.

#### Scenario: A hidden session wakes nothing
- **WHEN** a session's turn runs and the workspace shows another buffer
- **THEN** no deadline is set for it

#### Scenario: Shown again it is current
- **WHEN** the session has been hidden for 60s and is shown
- **THEN** its line reads the turn's full duration

### Requirement: Replayed history shows no times
Updates that arrive before the answer to `session/load` SHALL show no clock time, thought header or duration.

#### Scenario: A loaded session
- **WHEN** a session is loaded and its history replays messages, thoughts and finished tool calls
- **THEN** none of them shows a time; updates after the answer do

### Requirement: A command's open body is cut at 20 lines
An open call that is of kind `execute` or has a terminal SHALL show at most its first 20 output lines; when there are more, the next line SHALL read `… <N> more lines` (`1 more line` for one) in `annotation`, N counting the hidden output lines. The hidden lines SHALL stay in the buffer's text, invisible. `$ <command>` lines SHALL NOT count.

#### Scenario: A long output
- **WHEN** a command with 50 output lines is opened
- **THEN** `$ <command>`, lines 1 to 20 and `… 30 more lines` in `annotation` show, and the buffer's text still holds all 50 lines

#### Scenario: Exactly 20 lines
- **WHEN** a command with 20 output lines is opened
- **THEN** all 20 show and no count does

#### Scenario: Output streams into an open call
- **WHEN** an open command's output grows past 20 lines
- **THEN** the shown lines stay the first 20 and the count follows

#### Scenario: Other kinds are uncut
- **WHEN** a `read` call without a terminal has 50 lines of output and is opened
- **THEN** all 50 show

### Requirement: RET on a header opens the full output
`RET` in a session buffer with point on a tool call's header line SHALL show the call's full output in a read-only buffer of its own, in another window, instead of sending the prompt: its terminal's output with its faces, else its diffs in the `diff-*` faces, else its whole `rawOutput`, else its text items. The buffer SHALL be named `*<agent> output: <target>*<<workspace>>`, made unique, and SHALL be reused and refreshed by a later `RET` on that header. With none of those it SHALL echo `No output` and send nothing. This SHALL work whether the call is folded, while a turn runs and after the session ended.

#### Scenario: Opening a command's output
- **WHEN** point is on the header of a command with 50 output lines and `RET` is typed
- **THEN** a read-only buffer holds all 50 lines, without escapes, and the prompt is not sent

#### Scenario: RET again
- **WHEN** the output grew and `RET` is typed on the header again
- **THEN** the same buffer shows the new output

#### Scenario: A killed output buffer
- **WHEN** the output buffer was killed and `RET` is typed on the header
- **THEN** a new one opens

#### Scenario: No output
- **WHEN** `RET` is typed on the header of a call with no output
- **THEN** `No output` is echoed and nothing is sent

#### Scenario: Elsewhere
- **WHEN** `RET` is typed in the prompt area
- **THEN** the prompt is sent as before

### Requirement: Consecutive tool calls fold into a group
A run of tool calls SHALL show as one group once it holds two calls. A run SHALL end at an agent message, a user prompt, a line of the session's own or the first show of the plan; an update to a plan already shown SHALL NOT end it. Thoughts and permission blocks SHALL NOT end a run: those after its first call, and thoughts before it, SHALL be rows of the group. A run with one call SHALL show as it would alone, with no group line, its thoughts as ordinary blocks. A group SHALL start folded, showing only its group line, unless its first call was already open, in which case it starts open. It SHALL stay folded or open as it grows and as its calls update, and each call SHALL stay folded or open.

Rows SHALL sit on consecutive lines, without blank lines between them, and each tool call or thought row SHALL be indented two columns. A blank line SHALL follow the group's last row.

#### Scenario: Two calls fold
- **WHEN** two tool calls arrive in a row
- **THEN** one group line shows and neither call's row does

#### Scenario: One call has no group line
- **WHEN** one tool call arrives and an agent message follows
- **THEN** the call shows as its header with no group line

#### Scenario: A message starts a new group
- **WHEN** two calls, an agent message and two more calls arrive
- **THEN** two group lines show with the message between them

#### Scenario: A plan update doesn't split a group
- **GIVEN** a plan is shown
- **WHEN** a call, a plan update and a second call arrive
- **THEN** one group holds both calls

#### Scenario: A thought goes inside
- **WHEN** a thought, a call, a thought and a call arrive
- **THEN** one group holds all four as rows, in order

#### Scenario: A growing group stays folded
- **WHEN** a third call and updates to the first two arrive at a folded group
- **THEN** only the group line shows

#### Scenario: An open call stays open as its run becomes a group
- **WHEN** the single call of a run is opened and a second call arrives
- **THEN** the group is open with the first call's content showing

### Requirement: The group line
The group line SHALL read `<fold> <icon> <verbs>` in `session-thought`, `▸` when folded and `▾` when open, with no rule beside it. The verbs SHALL be one phrase per class of kind in order of first appearance, comma separated, only the first capitalized: `read files`, `searched code`, `edited files` (edit, delete and move), `ran commands`, `fetched pages`, `used tools` (anything else). A class with an unfinished call SHALL read in the present tense: `reading files`, `searching code`, `editing files`, `running commands`, `fetching pages`, `using tools`. The icon SHALL be the first call's. Right-aligned, it SHALL show `<n> failed` in `status-failed` when calls failed, and, once every call has finished, the time from the first call's arrival to the last one's finish in `annotation`, two spaces after any failure count. A replayed call SHALL leave the duration out.

#### Scenario: Verbs follow first appearance
- **WHEN** a group's finished calls are a read, an execute and a read
- **THEN** its line reads `Read files, ran commands`

#### Scenario: The running verb is present
- **WHEN** a group holds a finished edit and an unfinished execute
- **THEN** its line reads `Edited files, running commands` and has no duration

#### Scenario: Failures show when folded
- **WHEN** one call of a folded group fails and the others finish
- **THEN** its line ends with `1 failed` in `status-failed` then the duration

#### Scenario: Duration spans the group
- **WHEN** a call arrives, a second arrives 1s later and it finishes 2s after that, the first having finished
- **THEN** the group line shows `3.0s`

### Requirement: Thought rows
A thought in a group SHALL show as one row, `  ▸ <first line>` in `session-thought`, where the first line is its `Thought for <n>s` line, or its text's first line when replayed; the rest SHALL be folded under it. While it streams it SHALL show as a thought block does, in an open group.

#### Scenario: A thought row folds its text
- **WHEN** a group holding a thought `hmm` is opened
- **THEN** the row reads `  ▸ Thought for 0s` and `hmm` shows only after `TAB` on the row

### Requirement: Waiting calls show under a folded group
While a permission request is pending for a call of a folded group, that call's row SHALL show under the group line. A pending permission block in a group SHALL show too. Once answered, the answered line SHALL stay in the group as a row and hide with the rest.

#### Scenario: Waiting under the fold
- **WHEN** a folded group's second call gets a permission request
- **THEN** the group line, the call's row and the request's block show, and not the first call

#### Scenario: Answering folds back
- **WHEN** that request is answered
- **THEN** only the group line shows, and opening the group shows the answered line under the call

### Requirement: Motion across folded groups
`C-n` and `C-p` SHALL pass a folded group as one line, as they pass a folded call. Search SHALL still match its hidden text and kill and copy SHALL still yield it. `RET` SHALL show a call's output only from a row that shows.

#### Scenario: Line motion
- **WHEN** point is on the line above a folded group and `C-n` is typed twice
- **THEN** point is on the group line, then on the line after the group

#### Scenario: Search reaches hidden rows
- **WHEN** `C-s` searches for a hidden call's target
- **THEN** it matches

#### Scenario: A killed region holds hidden rows
- **WHEN** a region spanning a folded group is copied
- **THEN** the copy holds every row's text

### Requirement: Replayed history groups
Calls replayed by `session/load` SHALL group as live ones do, without durations.

#### Scenario: A loaded session
- **WHEN** a load replays two finished calls
- **THEN** one group line shows with no duration

### Requirement: File links in agent messages
In an agent message, a whitespace-delimited `path`, `path:line` or `path:line:col` reference, with surrounding quotes, backticks, brackets, `*`, `,` and `;` and a trailing `.`, `:`, `!` or `?` left out, SHALL be drawn in the `link` face, underlined, when the path names a file in the project's file index: relative to any root, primary first, or absolute under a root. Any other reference SHALL stay as it is. Thoughts, the echoed prompt and tool call bodies get no links.

#### Scenario: An existing path
- **WHEN** an agent message says ``see `src/a.rs:12` for it`` and `src/a.rs` is in the primary root
- **THEN** `src/a.rs:12` is in the `link` face

#### Scenario: A missing path
- **WHEN** an agent message names `src/gone.rs` and no root holds it
- **THEN** it stays plain text

#### Scenario: Another root
- **WHEN** an agent message names `lib/b.rs`, which exists only under a second root
- **THEN** it is a link to that root's file

### Requirement: File links in tool call headers
The path a tool call's header shows, from its first location or its diff, SHALL be drawn in the `link` face, underlined, when that file is in the project's file index. A header showing a command, URL or title has no link.

#### Scenario: A read of an existing file
- **WHEN** a read call's location is a file under the primary root
- **THEN** the path on its header is in the `link` face

#### Scenario: A file outside the project
- **WHEN** a read call's location is `/etc/hosts`
- **THEN** the header shows it without the `link` face

### Requirement: RET on a link visits the file
`RET` in a session buffer with point on a link, above the prompt line, SHALL visit the linked file in another window, splitting a lone window, and put point at the start of the link's line: the reference's `line`, or a tool call location's `line`, both counted from 1, else the file's start. It SHALL send nothing. `RET` on a tool call's header outside its link still opens the full output.

#### Scenario: Visiting a referenced line
- **WHEN** point is on the link `src/a.rs:3` in an agent message and `RET` is typed
- **THEN** `src/a.rs` shows in another window with point on line 3 and the prompt is not sent

#### Scenario: Visiting a tool call's location
- **WHEN** point is on the path of a read call's header and `RET` is typed
- **THEN** that file shows in another window

#### Scenario: Elsewhere on a header
- **WHEN** point is on a header's verb and `RET` is typed
- **THEN** the call's full output opens as before

### Requirement: Bare URLs are links
Wherever session text is rendered as markdown, outside code spans and code blocks, a run starting with `http://` or `https://` at the start of a word SHALL be drawn in the `link` face, underlined, as autolinks are. The run SHALL end at whitespace, quotes, backticks, angle or square brackets or braces, without trailing `.`, `,`, `:`, `;`, `!`, `?`, `*`, `_` or `~`, and without a trailing `)` that has no `(` to match it inside the URL. Its text SHALL stay visible and unchanged.

#### Scenario: A bare URL in an agent message
- **WHEN** an agent message says `See https://x.dev/docs.`
- **THEN** `https://x.dev/docs` is in the `link` face and the trailing `.` is not

#### Scenario: Another scheme
- **WHEN** an agent message says `ftp://x.dev/file`
- **THEN** it stays plain text

#### Scenario: Parentheses
- **WHEN** an agent message says `(https://en.wikipedia.org/wiki/Emacs_(text_editor))`
- **THEN** `https://en.wikipedia.org/wiki/Emacs_(text_editor)` is in the `link` face and the outer parentheses are not

#### Scenario: Inline code
- **WHEN** an agent message says `` `https://example.org/not-a-link` ``
- **THEN** the URL is not in the `link` face and `RET` on it does not open it

### Requirement: RET on a URL opens it in the browser
`RET` in a session buffer with point on a URL link above the prompt line SHALL open the URL in the system's default browser and send nothing. A URL link is the visible text of a markdown inline link whose destination is the URL, an autolink, or a bare URL; the URL opened is the link's destination, the autolink's text without its `<` and `>`, or the bare word. Only `http` and `https` URLs SHALL open; `RET` on a link to any other scheme SHALL echo `Not a web link: <url>` and send nothing. This check SHALL come after chips and before file links, tool call headers and sending, and SHALL work while a turn runs and after the session ended.

#### Scenario: A markdown link
- **WHEN** an agent message reads `See [docs](https://x.dev).`, point is on `docs` and `RET` is typed
- **THEN** `https://x.dev` opens in the default browser and the prompt is not sent

#### Scenario: An autolink
- **WHEN** point is on the autolink `<https://x.dev/a>` and `RET` is typed
- **THEN** `https://x.dev/a` opens in the default browser

#### Scenario: A bare URL while a turn runs
- **WHEN** a turn runs, point is on the bare URL `https://x.dev/b` in an earlier message and `RET` is typed
- **THEN** `https://x.dev/b` opens in the default browser and nothing echoes `is busy`

#### Scenario: Another scheme
- **WHEN** point is on the text of `[run](file:///bin/sh)` and `RET` is typed
- **THEN** nothing opens and the echo area shows `Not a web link: file:///bin/sh`

#### Scenario: In the prompt area
- **WHEN** the prompt area holds `https://x.dev` with point on it and `RET` is typed
- **THEN** the prompt is sent as before

### Requirement: File chips
A chip SHALL be one char of a session buffer's text naming a file, a directory or an image, drawn as a space, its icon (`▤` file, `▣` image, `▦` directory), a space, the path's last component and a space, in the boxed `attachment` face. Motion, deletion, kill, yank and undo SHALL treat it as one char, and a yanked chip SHALL keep its path. Word motion and word kills SHALL take a chip as a word of its own. While point is on a chip, the cursor SHALL cover the whole chip.

#### Scenario: Drawn as icon and name
- **WHEN** a chip for `/p/src/main.rs` is in the prompt
- **THEN** it is drawn as ` ▤ main.rs ` in the `attachment` face

#### Scenario: One char
- **WHEN** point is before a chip and `C-f` runs, then `DEL`
- **THEN** point moves past the chip, then the chip is deleted whole

#### Scenario: A word of its own
- **WHEN** point is at the start of `ab <chip>cd` and `M-f` runs three times
- **THEN** point stops after `ab`, after the chip, then after `cd`

#### Scenario: Undo brings it back
- **WHEN** a deleted chip's deletion is undone
- **THEN** the chip is drawn again as before

### Requirement: Chips are sent as resource links and images
Sending a prompt with chips SHALL send its text as text blocks split at the chips, runs of only whitespace left out, and each chip in its place: a file or directory as a `resource_link` with the `file://` URI of its path, a directory's ending in `/`, and an image (`png`, `jpg`, `jpeg`, `gif`, `webp`) as image content with the file's data, read when it is sent. An image that can't be read SHALL end the turn with `Error: <reason>`.

#### Scenario: A file in the middle
- **WHEN** the prompt is `look at <chip /p/a.rs> please` and is sent
- **THEN** the request's blocks are the text `look at `, a resource link `a.rs` to `file:///p/a.rs` and the text ` please`

#### Scenario: An image
- **WHEN** the prompt holds a chip for `/p/cat.png` and is sent
- **THEN** the request carries image content of type `image/png` with the file's bytes

### Requirement: Chips in sent and replayed messages
The echoed prompt SHALL show its chips as the prompt did. In a replayed user message, each `[@name](file://path)` SHALL show as a chip for `path`, a directory when it ends in `/`; each `@path` word at the start or after whitespace, as the Claude Code CLI writes a mention, SHALL show as a chip when `path` is a file in the project's file index, relative to any root or absolute under one; and an image block SHALL show as an `▣ image` chip. Replayed user chunks with the same message id SHALL join one block.

#### Scenario: Echo
- **WHEN** a prompt with a chip is sent
- **THEN** the echo above the new prompt line shows the same chip

#### Scenario: Replay
- **WHEN** a loaded session replays the chunks `look at `, `[@a.rs](file:///p/a.rs)` and ` please` with one message id
- **THEN** one block shows `> look at  ▤ a.rs  please`, the middle a chip

#### Scenario: A CLI mention
- **WHEN** a loaded session replays `@src/a.rs vs @src/gone.rs`, and only `src/a.rs` is in the index
- **THEN** `@src/a.rs` shows as a chip and `@src/gone.rs` stays text

### Requirement: RET and the echo on a chip
`RET` with point on a chip SHALL visit its file in another window, splitting a lone one, open an image in the system's default app, or for a directory open `find-file` in it, sending nothing. A chip without a path SHALL echo `No file`, and one whose file is gone `<path>: no such file`, without splitting. While point is on a chip, the echo line SHALL show its full path when the command left no other message.

#### Scenario: Visiting
- **WHEN** point is on a chip for `/p/a.rs` in the prompt and `RET` is typed
- **THEN** `/p/a.rs` shows in another window and the prompt is not sent

#### Scenario: An image
- **WHEN** point is on a chip for `/p/cat.png` and `RET` is typed
- **THEN** the system opens `/p/cat.png` and no window splits

#### Scenario: The path on the echo line
- **WHEN** point moves onto a chip for `/p/a.rs`
- **THEN** the echo line shows `/p/a.rs`

### Requirement: Missing files are struck through
A chip whose file is gone from disk SHALL be drawn struck through, its icon in `status-failed`, wherever it shows. The state SHALL be checked when its path is first seen and when the file watcher reports it, and SHALL clear when the file is back.

#### Scenario: Deleted
- **WHEN** a check answers that a chip's file is gone
- **THEN** the chip is struck through with its icon in `status-failed`

### Requirement: Slash commands from the agent
The core SHALL keep each session's command list from its latest `available_commands_update`, each update replacing the previous list. Commands the editor does itself SHALL be left out: `plan`.

#### Scenario: The list follows updates
- **WHEN** a later `available_commands_update` lists only `compact`
- **THEN** `Command: ` lists only `compact`

#### Scenario: Commands the editor does are hidden
- **WHEN** the update lists `plan` and `mcp`
- **THEN** `Command: ` lists only `mcp`

### Requirement: Picking a slash command
`agent-command` SHALL read `Command: ` completing over the current session's commands, each annotated with its argument hint, two spaces and its description's first non-blank line cut to 60 chars. A picked command SHALL be inserted as a command chip and a space at point in the prompt area, or at the prompt area's end from the transcript, a space first when that end follows text; point SHALL go after it. An answer naming no command SHALL be inserted as `/<answer> ` text the same way.

#### Scenario: Slash opens the commands
- **WHEN** the agent has sent `available_commands_update` with `review` (hint `[path]`, description `Review changes`) and `/` is typed at the start of an empty prompt area
- **THEN** `Command: ` opens listing `review` annotated `[path]  Review changes`

#### Scenario: Picking inserts a chip
- **WHEN** `review` is picked
- **THEN** the prompt area holds the `review` command chip and a space, drawn ` ▷ /review `, and point is at its end

#### Scenario: Mid-message
- **WHEN** the prompt area holds `fix it` and `/` is typed after ` ` at its end and `review` picked
- **THEN** the prompt area reads `fix it <review chip> `

### Requirement: Slash at a word's start
`/` in the prompt area at its start or after whitespace, with a non-empty command list, SHALL run `agent-command`; otherwise it SHALL insert itself.

#### Scenario: Slash inside a word types itself
- **WHEN** `/` is typed right after a non-space char in the prompt area, or in a session with no commands
- **THEN** `/` is inserted and no prompt opens

#### Scenario: From the transcript
- **WHEN** the prompt area holds `work`, point is in the transcript and `M-x agent-command` picks `compact`
- **THEN** the prompt area reads `work <compact chip> ` and point is at its end

### Requirement: Command chips
A command chip SHALL be a chip, one char edited as file chips are, drawn as a space, `▷`, a space, `/` and the command's name, and a space, in the boxed `attachment` face. On send it SHALL go as the text `/name` within the prompt's text. `RET` on it SHALL send the prompt. A sent or replayed user message's `/name` at a word's start SHALL show as a command chip when the session's command list names it.

#### Scenario: Sent as text
- **WHEN** the prompt area holds the `review` chip, a space and `x`, and `RET` is pressed with point on the chip
- **THEN** the prompt is sent as the text `/review x`

#### Scenario: Replayed commands
- **WHEN** the session knows `review` and a replayed user message reads `/review it, not /x`
- **THEN** `/review` shows as a command chip and `/x` stays text

### Requirement: Project references in the prompt
Typing `@` in a session's editable prompt SHALL insert the literal `@` and open `Reference in <project>: ` with fuzzy completion over that project's indexed files and directories in every root. Candidates SHALL use the project finder's root labels, including duplicate-name suffixes; directories SHALL end in `/`, use the directory face and show `dir`, while files SHALL show their modification age. The session SHALL remain visible while choosing. First use SHALL start indexing when necessary and fill the open list when a compatible result arrives. The picker SHALL keep its captured root mapping for its lifetime.

#### Scenario: Every root and directory
- **GIVEN** a project with roots `/a/app` and `/b/app`, indexed files `src/main.rs` and `lib.rs`, and directories `src` and `empty`
- **WHEN** `@` is typed in the session prompt
- **THEN** `Reference in <project>: ` offers `app/src/main.rs`, `app/src/`, `app<2>/lib.rs` and `app<2>/empty/`, and the session stays visible

#### Scenario: First-use indexing
- **GIVEN** the project has no index
- **WHEN** `@` opens the picker and the index then answers
- **THEN** the open list gains the indexed files and directories without reopening

#### Scenario: Roots change during picking
- **GIVEN** the picker offers `app/src/main.rs` from `/old/app`
- **WHEN** the project's roots change to `/new/app` before that candidate is picked
- **THEN** the inserted chip still refers to `/old/app/src/main.rs`

#### Scenario: Candidate annotations
- **GIVEN** an indexed file with a known modification age and an indexed directory
- **WHEN** their candidates appear in the reference picker
- **THEN** the file shows its age and the directory shows `dir`

### Requirement: A reference replaces its triggering at sign
Picking a file or directory SHALL replace only the triggering `@` with its existing file, image or directory chip and place point after it. `RET` on a directory candidate SHALL attach it rather than enter it. `C-g` SHALL return to the session with the literal `@`, surrounding text and point preserved. An answer without a candidate SHALL require a match and leave the picker open. Input in the read-only transcript SHALL retain the read-only protection.

#### Scenario: Insert amid text
- **GIVEN** the prompt contains `compare  please` with point between its spaces
- **WHEN** `@` is typed and an indexed file is picked
- **THEN** the prompt contains `compare <file chip> please`, and sending includes the referenced file as a resource link

#### Scenario: Attach a directory
- **GIVEN** a directory candidate ending in `/` is selected
- **WHEN** `RET` is pressed
- **THEN** the prompt's `@` becomes a directory chip, sent as a directory resource link

#### Scenario: Cancel while output arrives
- **GIVEN** `@` opened the picker amid prompt text
- **WHEN** agent output arrives above the prompt and `C-g` is pressed
- **THEN** the prompt keeps the literal `@` and its surrounding text, with point after `@`

#### Scenario: Match required
- **GIVEN** the reference picker is open
- **WHEN** text matching no indexed candidate is submitted
- **THEN** the picker remains open and reports `Match required`

#### Scenario: Read-only transcript
- **GIVEN** point is in the read-only transcript
- **WHEN** `@` is typed
- **THEN** the editor reports `Text is read-only` and preserves the transcript

### Requirement: Attach a disk path to the prompt
`agent-attach-file` SHALL read `Attach: ` over paths anywhere on disk, using find-file's starting directory, asynchronous filesystem completion, annotations and directory editing. The session SHALL remain visible without a file preview. A picked file, image or directory SHALL insert its existing chip at point in the editable prompt, with point after it and surrounding text preserved. From the transcript, it SHALL append at the prompt's end. Cancellation SHALL preserve the prompt and its point, including when output arrives during picking. Sending SHALL use existing chip resource-link and image behavior.

#### Scenario: An external file amid text
- **GIVEN** the prompt contains `compare  please` with point between the spaces
- **WHEN** `agent-attach-file` picks `/outside/a.rs`
- **THEN** the session shows `compare <file chip> please`, and sending includes the resource link `file:///outside/a.rs` between the text blocks

#### Scenario: An image
- **GIVEN** a session prompt
- **WHEN** `agent-attach-file` picks `/screenshots/view.png`
- **THEN** an image chip is inserted, and sending carries the existing image content for that path

#### Scenario: From the transcript
- **GIVEN** point is in the transcript and the prompt contains `work`
- **WHEN** `agent-attach-file` picks a file
- **THEN** its chip is appended after `work` and point follows it

#### Scenario: Cancel after output
- **GIVEN** the attachment picker opened amid prompt text
- **WHEN** agent output arrives above the prompt and `C-g` is pressed
- **THEN** the original prompt and its relative point are preserved

### Requirement: Navigate or attach a directory
In `Attach: `, `RET` on a selected directory SHALL enter it and keep the picker open. `C-RET` SHALL attach the selected file or directory as it is, closing the picker. With no selection, `C-RET` SHALL attach the typed path, with a trailing slash denoting a directory, so the current directory can be attached. `RET` on a file SHALL attach it. The extra binding SHALL be local to the attachment picker; other filesystem pickers SHALL keep their current behavior.

#### Scenario: Enter then attach a directory
- **GIVEN** the attachment picker selects directory `/outside/screens/`
- **WHEN** `RET` is pressed, then `C-RET` with no selection
- **THEN** `RET` lists that directory's entries and `C-RET` inserts its directory chip, sent as `file:///outside/screens/`

#### Scenario: Attach the selected directory directly
- **GIVEN** directory `/outside/empty/` is selected in `Attach: `
- **WHEN** `C-RET` is pressed
- **THEN** its directory chip is inserted without entering it

#### Scenario: Attach a selected file
- **GIVEN** a file candidate is selected in `Attach: `
- **WHEN** `C-RET` is pressed
- **THEN** its file chip is inserted

### Requirement: Long prompt yanks fold
A yank into the session prompt strictly longer than `paste_fold_lines` SHALL appear as ` ≡ Pasted N lines ` in the boxed attachment face. The complete original text SHALL remain in the buffer. Yanks at or below the limit and yanks into ordinary buffers SHALL remain plain. A trailing newline SHALL not count as an extra line. Yank-pop SHALL apply the same threshold to the replacement yank.

#### Scenario: Long yank
- **GIVEN** the default threshold and a 21-line clipboard
- **WHEN** it is yanked into the session prompt
- **THEN** the prompt displays `≡ Pasted 21 lines` and its buffer contains the complete pasted text

#### Scenario: At the threshold
- **GIVEN** the default threshold and a 20-line clipboard
- **WHEN** it is yanked into the session prompt
- **THEN** all 20 lines remain plain text

### Requirement: TAB previews a paste
TAB on a folded paste SHALL retain its chip header and show the first four original lines below it in the session-block face, followed by `… N more lines` in the annotation face when additional lines remain. For fewer than four lines it SHALL show all available lines. TAB in the paste SHALL collapse it again and place point at its start. This behavior SHALL take precedence over tool folding only within a paste.

#### Scenario: Preview and collapse
- **GIVEN** a folded 21-line paste
- **WHEN** TAB is pressed at its start and then again inside it
- **THEN** the chip shows four preview lines and `… 17 more lines`, then folds again with point at its start

### Requirement: Folded pastes send complete input
Sending a prompt SHALL transmit all its pasted text in order with surrounding text and other chips. The sent transcript SHALL show each paste folded, including one whose preview was open before sending. New prompt text and streamed output SHALL not alter the pasted input or lose its fold.

#### Scenario: Send an open preview
- **GIVEN** a long paste surrounded by ordinary prompt text with its preview open
- **WHEN** the prompt is sent
- **THEN** the request contains all original pasted and surrounding text and the transcript shows a folded paste above the new prompt

#### Scenario: Output arrives while composing
- **GIVEN** a folded paste in the prompt
- **WHEN** streamed agent output is inserted above it and surrounding prompt text is edited
- **THEN** its fold follows its text and sending still includes every pasted line

### Requirement: Prompts are drawn as frame 6 draws them
Every line of a sent or replayed user message's block, the blank line after it excluded, SHALL have a `session-user` background across the window. Its first line SHALL have a 1 px border at its top and its last line one at its bottom, across the window, in the `session-user-rule` foreground. The message's text SHALL be in the `session-user` face, with chips, links, code and its time keeping their own faces over it. The `> ` that starts a sent or replayed message and the live prompt line SHALL be drawn as `❯ ` in `status-info`, bold. The buffer's text SHALL keep `> `, so motion, search, copy and kill see `> `.

#### Scenario: A sent prompt
- **WHEN** `a C-j b RET` is typed in a ready session
- **THEN** the echo's two lines have the `session-user` background, the first a top border and the second a bottom border in `session-user-rule`, `a` and `b` are in `session-user`, its first line is drawn starting `❯ ` in `status-info` and bold, and the buffer's text holds `> a\nb`

#### Scenario: A one-line prompt
- **WHEN** `a RET` is typed
- **THEN** the echo's one line has both borders

#### Scenario: The live prompt
- **WHEN** a session buffer is shown
- **THEN** its prompt line is drawn starting `❯ ` with no `session-user` background and no borders

### Requirement: The prompt area is docked at the window's bottom
A window showing a session buffer SHALL draw the prompt area, from the live prompt line to the end of the buffer, in a pane pinned to the bottom of its text area, and the transcript in the rows above it. The pane SHALL have as many rows as the prompt area takes, wrapped rows included, up to a third of the window's text rows and at least one; past that the pane SHALL scroll to keep point visible. With point in the prompt area the transcript rows SHALL show the transcript's last lines; with point in the transcript the transcript rows SHALL scroll to keep point visible, as a window does. Transcript text SHALL never show in the pane, nor the prompt area in the transcript rows. Windows showing other buffers SHALL be unchanged.

#### Scenario: A short transcript
- **WHEN** a session with a three-line transcript is shown in a window of 30 text rows
- **THEN** the prompt line is drawn in the window's last text row, under the pane's rule, and the transcript in the first rows

#### Scenario: Output arrives while typing
- **WHEN** point is in the prompt area and more transcript lines arrive than the transcript rows hold
- **THEN** the transcript rows show the transcript's last lines and the prompt pane is unchanged

#### Scenario: Reading the transcript
- **WHEN** point moves to the transcript's first line with `M-<`
- **THEN** the transcript rows show the transcript from its first line and the prompt pane still shows the prompt area

#### Scenario: A long prompt
- **WHEN** the prompt area holds 20 lines in a window of 30 text rows
- **THEN** the pane has 10 rows, showing point's line

### Requirement: Selecting the session's model
`agent-select-model` in a session buffer SHALL ask `Model: ` over the values of the session's model option, the first select config option of category `model`, by display name with groups flattened, the current value selected when the list opens and each value's description as its annotation. The options are the latest reported, as for `S-TAB`. Picking a name SHALL send `session/set_config_option` with the option's id and the first value with that name, and the mode line SHALL follow the answer.

#### Scenario: Picking a model
- **WHEN** the model option is `opus` with values `Opus` (`opus`) and `Sonnet` (`sonnet`), and `agent-select-model` is run and `Sonnet` picked
- **THEN** the list shows `Opus` selected and `Sonnet`, `model=sonnet` is sent, and after the answer the mode line shows `Sonnet`

#### Scenario: Grouped values
- **WHEN** the model option's values are in groups
- **THEN** the list shows every group's values in order

### Requirement: Model selection refusals
`agent-select-model` SHALL echo `<agent> has no model choice` in a session without a model option, `<agent> is not ready` before `session/new` is answered and `<agent> session has ended` after the agent exits; then no prompt opens and nothing is sent. A name no value has SHALL echo `<agent> has no model <name>` and send nothing. If the agent answers with an error, `*Messages*` SHALL get `<agent> could not set model <value>: <reason>` with the value id sent.

#### Scenario: No model option
- **WHEN** the session has no model option and `agent-select-model` is run
- **THEN** `claude has no model choice` is echoed, no prompt opens and nothing is sent

#### Scenario: A refused model
- **WHEN** the agent answers the set to `sonnet` with an error
- **THEN** `*Messages*` has `claude could not set model sonnet: ` followed by the reason

### Requirement: Selecting the session's effort
`agent-select-effort` in a session buffer SHALL ask `Effort: ` over the values of the session's effort option, the first select config option of category `thought_level`, by display name with groups flattened, the current value selected when the list opens and each value's description as its annotation. The values SHALL be the latest the agent reported, so after a model change they are the new model's. Picking a name SHALL send `session/set_config_option` with the option's id and the first value with that name, and the mode line SHALL follow the answer. It SHALL echo `<agent> has no effort choice` in a session without an effort option, `<agent> is not ready` before `session/new` is answered and `<agent> session has ended` after the agent exits, and then open no prompt and send nothing. A name no value has SHALL echo `<agent> has no effort <name>`. If the agent answers with an error, `*Messages*` SHALL get `<agent> could not set effort <value>: <reason>`.

#### Scenario: Picking an effort
- **WHEN** the effort option is `high` with values `Low` (`low`) and `High` (`high`), and `agent-select-effort` is run and `Low` picked
- **THEN** the list shows `Low` and `High` selected, `effort=low` is sent, and after the answer the mode line shows `Low`

#### Scenario: Efforts follow the model
- **WHEN** a model change reports an effort option with the values `Low` and `Max`
- **THEN** `agent-select-effort` lists `Low` and `Max`

#### Scenario: No effort option
- **WHEN** the session has no effort option and `agent-select-effort` is run
- **THEN** `claude has no effort choice` is echoed, no prompt opens and nothing is sent

### Requirement: Tables render as drawn text
A GFM pipe table at the top level of an agent message block SHALL be replaced in the buffer by drawn text: one line for the header and one for each body row, without the delimiter row. Each cell SHALL be a space, its text padded to its column's width and a space. A column's width SHALL be its widest cell's shown text, measured in chars. Cells SHALL align left, centre or right as the delimiter row says. Each line SHALL carry the table line decoration.

#### Scenario: Columns and alignment
- **WHEN** the agent streams `| a | b |\n|---|--:|\n| x | 10 |\n` and a frame is built
- **THEN** the block's lines read ` a   b ` and ` x  10 `, and both carry the table decoration

#### Scenario: A ragged table
- **WHEN** a table's header has three cells, one body row two and another four
- **THEN** each drawn line has three cells: the short row ends in an empty cell and the long row's fourth cell is dropped

#### Scenario: A nested table stays text
- **WHEN** the agent streams a table inside a block quote or a list item
- **THEN** its lines stay as written, pipes and delimiter row included

#### Scenario: A table still streaming
- **WHEN** a table's header and delimiter row have arrived, a frame is built, then a row arrives and another frame is built
- **THEN** the first frame shows the header line alone, and the second also shows the row, with columns widened to fit it

### Requirement: Table cells render their markup
Inline markup in a table cell SHALL be rendered with the faces messages use and left out of the drawn text and of the column's width; block markup in a cell SHALL stay as written. The header line SHALL be bold on the `session-block` background across the table's width. Links in cells SHALL be drawn and open with `RET` as in other message text; a markdown link SHALL show only its label, and `RET` on the label SHALL act on its destination.

#### Scenario: Markup in a cell
- **WHEN** a table's cell holds `**hot** path` and the column's other cells are shorter
- **THEN** the cell's drawn text is `hot path` with `hot` bold, and the column is 8 chars wide

#### Scenario: Block markers in a cell
- **WHEN** a table's cells hold `#`, `# *x*` and a code fence
- **THEN** they draw as `#`, `# x` with `x` italic, and the fence as written

#### Scenario: The header
- **WHEN** a table is drawn
- **THEN** its header line's text is bold and has the `session-block` background from the table's first column to its last

#### Scenario: A link in a cell
- **WHEN** a table's cell holds `[docs](https://x.dev)` and `RET` is typed on `docs`
- **THEN** the cell's drawn text is `docs` in `link`, and `https://x.dev` opens in the system's browser

#### Scenario: A link in a cell to another scheme
- **WHEN** a table's cell holds `[sh](file:///bin/sh)` and `RET` is typed on `sh`
- **THEN** the echo area says `Not a web link: file:///bin/sh` and nothing opens

### Requirement: A message's time avoids its table rows
A live agent message's clock time SHALL go at the end of its first line that is neither blank nor a table row. A message whose lines are all table rows or blank SHALL get no time.

#### Scenario: A reply starting with a table
- **WHEN** a live reply `| a |\n|---|\n| b |\n\nAfter.` ends at 14:01:00
- **THEN** its lines read ` a `, ` b `, an empty line and `After. 14:01:00`

#### Scenario: A reply of a table alone
- **WHEN** a live reply `| c |\n|---|\n| d |` ends
- **THEN** its lines read ` c ` and ` d ` with no time
