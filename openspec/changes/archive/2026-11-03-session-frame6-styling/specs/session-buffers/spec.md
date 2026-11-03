# Spec Delta

## ADDED Requirements

### Requirement: Prompts are drawn as frame 6 draws them
Every line of a sent or replayed user message's block, the blank line after it excluded, SHALL have a `session-user` background across the window. The `> ` that starts a sent or replayed message and the live prompt line SHALL be drawn as `❯ ` in `status-info`, bold. The buffer's text SHALL keep `> `, so motion, search, copy and kill see `> `.

#### Scenario: A sent prompt
- **WHEN** `a C-j b RET` is typed in a ready session
- **THEN** the echo's two lines have the `session-user` background, its first line is drawn starting `❯ ` in `status-info` and bold, and the buffer's text holds `> a\nb`

#### Scenario: The live prompt
- **WHEN** a session buffer is shown
- **THEN** its prompt line is drawn starting `❯ ` with no `session-user` background

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

## MODIFIED Requirements

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
