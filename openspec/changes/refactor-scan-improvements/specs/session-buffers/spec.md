# Spec Delta

## MODIFIED Requirements

### Requirement: Command output shows in its tool call
A `tool_call`, or a call started from an update, SHALL open a terminal for the call when its `_meta.terminal_info` names a `terminal_id` or a content item is a terminal, the first of these naming the terminal's id; a call keeps the terminal it opened when later updates replace its content. Every `tool_call` and `tool_call_update` SHALL append the `data` string of `_meta.terminal_output` and of `_meta.terminal_output_delta`, in that order, to the output of the terminal their `terminal_id` names, whichever call announced it. `_meta.terminal_exit` SHALL record that terminal's exit: `signal <signal>` when `signal` is a string, else `exit <code>` when `exit_code` is a non-zero number, else none. Data or an exit for a terminal no shown call opened SHALL be dropped.

The call's own terminal SHALL show as `Output:` and its output in a fenced code block, with ANSI escape sequences and carriage returns removed, in the place of `rawOutput` in the content, and its `terminal` content item SHALL NOT show as `Terminal <id>`. A terminal without output yet SHALL show nothing. The exit, when recorded, SHALL follow the status in the header, `session-thought` like it. An update that changes only a terminal's output or exit SHALL re-render that call when the next frame showing its session is built, once however many such updates came, and an update with fields SHALL rewrite the call at once. Frames that do not show the session SHALL defer that rendering while retaining the output and exit. A session is shown as for streamed messages.

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

#### Scenario: Hidden command output catches up
- **GIVEN** a session is hidden behind another buffer or belongs to an inactive workspace
- **WHEN** output-only updates arrive for its call while frames show other buffers
- **THEN** its block is unchanged until the first frame showing the session, which holds every chunk in order without needing another update

#### Scenario: Open and folded calls agree
- **GIVEN** two calls receive the same chunks, one open and one folded
- **WHEN** the frame is built and the folded call is opened
- **THEN** both blocks hold the same text and faces
