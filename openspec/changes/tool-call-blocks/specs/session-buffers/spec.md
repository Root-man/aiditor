# Spec Delta

## ADDED Requirements

### Requirement: A command's open body is cut at 20 lines
An open call that is of kind `execute` or has a terminal SHALL show at most its first 20 output lines; when there are more, the next line SHALL read `… <N> more lines` (`1 more line` for one), N counting the hidden output lines. The hidden lines SHALL stay in the buffer's text, invisible. `$ <command>` lines SHALL NOT count.

#### Scenario: A long output
- **WHEN** a command with 50 output lines is opened
- **THEN** `$ <command>`, lines 1 to 20 and `… 30 more lines` show, and the buffer's text still holds all 50 lines

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

## MODIFIED Requirements

### Requirement: Tool calls show folded
Each `tool_call` update SHALL add a block above the prompt line, after any block being streamed, separated from the next by a blank line. Its first line, the header, SHALL read `<fold> <status> <icon> <verb> <target>`: `▸` when folded or `▾` when open, in the `session-thought` face; `✓` for `completed` in `session-thought`, `✗` for `failed` in `status-failed` or `●` otherwise in `status-running`; the kind's icon, drawn over two spaces in `session-thought`: book for `read`, magnifier for `search`, pencil for `edit`, `delete` and `move`, terminal for `execute`, globe for `fetch`, wrench otherwise; the kind's verb in `session-thought`, in the past tense once `completed` or `failed` (`Read`, `Searched`, `Edited`, `Deleted`, `Moved`, `Ran`, `Fetched`, `Used`) and the present before that (`Reading`, `Searching`, `Editing`, `Deleting`, `Moving`, `Running`, `Fetching`, `Using`); and the target as plain text. A rule in the `session-thought` face SHALL be beside it. The target SHALL be the first of those that applies: a command's `rawInput.command` for an `execute` call or one with a terminal; `rawInput.url` for `fetch`; for `read`, `edit`, `delete` and `move`, the first location's path, else the first diff's, relative to the project's primary root when under it; else the title. A target SHALL be its first line, followed by ` …` when it has more, a title without surrounding backticks. When the call has content, the header SHALL be followed by a newline and the content, which SHALL start folded: invisible, with no placeholder.

The content SHALL hold, in order and each separated by a blank line: `Input:` and the call's whole `rawInput` as pretty-printed JSON in a fenced `json` code block, when it has one, it is not a command with `rawInput.command` and it has no diff item; each content item, a text block as its markdown, a resource link as its URI, a diff as its path and a unified diff in a fenced `diff` code block with added lines in `diff-added`, removed lines in `diff-removed` and the rest of the unified header in `diff-hunk`, a terminal other than the call's own terminal (see Command output shows in its tool call) as `Terminal <id>`, and any other item as `[<type>]`; and the output in a fenced code block: `$ <command>` above it for a command with `rawInput.command`, else `Output:` before the fence, holding the call's terminal output when it has a terminal, else the call's whole `rawOutput` when it has one and no content item, or, for a command, whatever its text items: a string as itself, a list of text blocks as their text joined, anything else as pretty-printed JSON. A command without output SHALL show `$ <command>` alone. Only a command's output is cut, when shown (see A command's open body is cut at 20 lines). A fence SHALL be longer than any run of backticks in what it encloses.

#### Scenario: A tool call starts folded
- **WHEN** the agent sends a `tool_call` titled `` `ls` `` of kind `execute`, status `pending` and `rawInput` `{"command":"ls"}`
- **THEN** a block reads `▸ ●`, the terminal icon, `Running ls`, then its content, of which nothing shows

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
- **THEN** its diff shows with added and removed lines in their faces, and no `Input:`

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
`TAB` in a session buffer (`agent-toggle-tool-call`) with point on a tool call's header line SHALL show its content if folded and hide it otherwise. With point anywhere in an unfolded call's content it SHALL hide the content and move point to the start of the header. The header's fold marker SHALL follow. Shown content SHALL be rendered as markdown, as messages are, with the rule beside each of its lines. With point anywhere else it SHALL echo `No tool call here`.

#### Scenario: Unfolding and folding
- **WHEN** point is on a folded call's header and `TAB` is typed, then `TAB` again
- **THEN** the content shows under the header with the rule beside it and its fences hidden, then hides again, the fold marker going from `▸` to `▾` and back

#### Scenario: Folding from inside the content
- **WHEN** a call is unfolded, point is on a line of its content and `TAB` is typed
- **THEN** the content hides and point is at the start of the header

#### Scenario: Not on a header
- **WHEN** `TAB` is typed in the prompt area
- **THEN** `No tool call here` is echoed and nothing changes

#### Scenario: A call without content
- **WHEN** `TAB` is typed on the header of a call without content and an update then brings output
- **THEN** the output arrives folded

#### Scenario: Unfolded content stays open
- **WHEN** a call is unfolded and an update brings new output
- **THEN** the call stays unfolded, showing the new output

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

### Requirement: Tool calls show how long they ran
When a live tool call first reaches `completed` or `failed`, its header SHALL show its duration right-aligned in `annotation`: the time since it arrived less the time it waited for permission, as `0.3s` under a minute and `2m 05s` from a minute on. A command that recorded an exit SHALL show that instead, and a permission wait SHALL show over both.

#### Scenario: A finished call shows its duration
- **WHEN** a tool call arrives and completes 0.3s later
- **THEN** its header ends with `0.3s`

#### Scenario: A permission wait is not run time
- **WHEN** a tool call arrives, waits 10s for permission and completes 2s after the answer
- **THEN** its header ends with `2.0s`
