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
`agent-command` SHALL read `Command: ` completing over the current session's commands, each annotated with its argument hint, two spaces and its description's first non-blank line cut to 60 chars. Its answer SHALL be inserted as `/<answer> ` at the start of the prompt area, with point after it. `/` with point at the start of the prompt area and a non-empty command list SHALL run `agent-command`; otherwise it SHALL insert itself. The prompt SHALL be sent as text.

#### Scenario: Slash opens the commands
- **WHEN** the agent has sent `available_commands_update` with `review` (hint `[path]`, description `Review changes`) and `/` is typed at the start of an empty prompt area
- **THEN** `Command: ` opens listing `review` annotated `[path]  Review changes`

#### Scenario: Picking inserts the command
- **WHEN** `review` is picked
- **THEN** the prompt area reads `/review ` and point is at its end

#### Scenario: Slash elsewhere types itself
- **WHEN** `/` is typed after text in the prompt area, or in a session with no commands
- **THEN** `/` is inserted and no prompt opens

#### Scenario: From anywhere in the session buffer
- **WHEN** point is in the transcript and `M-x agent-command` picks `compact`
- **THEN** the prompt area starts with `/compact ` and point is after it

#### Scenario: Slash before text in the prompt
- **WHEN** the prompt area holds `x`, point is at its start and `review` is picked after `/`
- **THEN** the prompt area reads `/review x`
