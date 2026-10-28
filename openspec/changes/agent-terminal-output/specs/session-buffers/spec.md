## MODIFIED Requirements

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

## ADDED Requirements

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
