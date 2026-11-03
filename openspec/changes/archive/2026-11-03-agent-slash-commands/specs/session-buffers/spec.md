## ADDED Requirements

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
