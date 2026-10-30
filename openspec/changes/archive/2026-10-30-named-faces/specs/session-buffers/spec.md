## REMOVED Requirements

### Requirement: Transcript colours follow the theme
**Reason**: Session buffers store face names, so no colours are rewritten on reload.
**Migration**: See "Transcripts are drawn in the current theme".

## ADDED Requirements

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

## MODIFIED Requirements

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

### Requirement: Tool calls show folded
Each `tool_call` update SHALL add a block above the prompt line, after any block being streamed, separated from the next by a blank line. Its first line, the header, SHALL read `<title> (<kind>, <status>)`, or `<title> (<kind>, <status>, <exit>)` for a failed command (see Command output shows in its tool call), with the title's first line rendered as markdown and followed by ` …` when the title has more lines, ` (<kind>, <status>)` in the `session-thought` face, and a rule in the `session-thought` face beside it; `kind` and `status` are the ACP names (`execute`, `in_progress`). When the call has content, the header SHALL be followed by a newline and the content, which SHALL start folded: invisible, shown as ` …` at the header's end.

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

### Requirement: The plan shows and is replaced in place
The first `plan` update of a session SHALL add a block above the prompt line, after any block being streamed, separated from the next by a blank line. The block SHALL read `Plan`, then one line per entry in order: the entry's content's first line, followed by ` …` when it has more lines, then ` (<status>, <priority>)` in the `session-thought` face, with the ACP names (`pending`, `in_progress`, `completed`; `high`, `medium`, `low`). Every line of the block SHALL have a rule in the `session-thought` face beside it. Each later `plan` update SHALL replace the block's entries where the block is, even with later blocks after it; the rest of the transcript and the prompt area SHALL be unchanged. A plan with no entries SHALL show the `Plan` line alone.

#### Scenario: A plan shows
- **WHEN** the agent sends a plan with `Read the code` (`in_progress`, `high`) and `Write tests` (`pending`, `medium`)
- **THEN** a block reads `Plan`, `Read the code (in_progress, high)`, `Write tests (pending, medium)` above the prompt line

#### Scenario: A new plan replaces the old one in place
- **WHEN** a plan is shown, the agent streams a message after it, then sends a plan with `Read the code` `completed` and a third entry `Ship` (`pending`, `low`)
- **THEN** there is one plan block, above the message, reading `Plan`, `Read the code (completed, high)`, `Write tests (pending, medium)`, `Ship (pending, low)`

#### Scenario: An empty plan
- **WHEN** the agent sends a plan with no entries after a plan with entries
- **THEN** the plan block reads `Plan` alone

### Requirement: Permission requests show inline
A `session/request_permission` from a session whose `session/new` was answered and whose agent hasn't exited SHALL add a block above the prompt line and stay pending: the line `Permission: <title>`, then a line per option in order, `<n>. <name> (<kind>)`, numbered from 1, with the ACP name of the kind in the `session-thought` colour and a `session-thought` rule beside every line of the block. The title is the first line of the request's tool call title (followed by ` …` if it has more), else that of the tool call shown with the same id, else the tool call id. Several requests MAY be pending at once, each in its own block.

#### Scenario: A request shows
- **WHEN** the agent asks permission for tool call `Edit a.txt` with options `Yes` (`allow_once`) and `No` (`reject_once`)
- **THEN** `Permission: Edit a.txt\n1. Yes (allow_once)\n2. No (reject_once)` appears above the prompt line and nothing is answered yet

#### Scenario: A request without a title
- **WHEN** the request's tool call has no title and no call with its id is shown
- **THEN** the header line is `Permission: ` followed by the tool call id

### Requirement: Command output shows in its tool call
A `tool_call`, or a call started from an update, SHALL open a terminal for the call when its `_meta.terminal_info` names a `terminal_id` or a content item is a terminal, the first of these naming the terminal's id; a call keeps the terminal it opened when later updates replace its content. Every `tool_call` and `tool_call_update` SHALL append the `data` string of `_meta.terminal_output` and of `_meta.terminal_output_delta`, in that order, to the output of the terminal their `terminal_id` names, whichever call announced it. `_meta.terminal_exit` SHALL record that terminal's exit: `signal <signal>` when `signal` is a string, else `exit <code>` when `exit_code` is a non-zero number, else none. Data or an exit for a terminal no shown call opened SHALL be dropped.

The call's own terminal SHALL show as `Output:` and its output in a fenced code block, with ANSI escape sequences and carriage returns removed, in the place of `rawOutput` in the content, and its `terminal` content item SHALL NOT show as `Terminal <id>`. A terminal without output yet SHALL show nothing. The exit, when recorded, SHALL follow the status in the header, `session-thought` like it. An update that changes only a terminal's output or exit SHALL re-render that call when the next frame is built, once however many such updates came, and an update with fields SHALL rewrite the call at once.

The SGR escapes removed from the terminal output and from text content items SHALL become faces on the chars they precede, shown when the call is open: foreground (`30`–`37`, `90`–`97`, `38;5;<n>` for n below 16) and background (`40`–`47`, `100`–`107`, `48;5;<n>`) in the theme's ANSI colours, bold (`1`), underline (`4`), and their resets (`0`, `39`, `49`, `22`, `24`). Other colours SHALL show in the default colour, and other attributes and escapes SHALL be ignored. The buffer's text SHALL hold no escapes, so copying the output yields plain text.

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
- **THEN** after the turn the session buffer's call reads `(execute, failed, exit 1)` and its output holds every chunk in order

