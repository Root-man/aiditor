## MODIFIED Requirements

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

## ADDED Requirements

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
A session buffer's mode line SHALL show ` [<mode id>]` after the buffer name once the session is ready and advertised a current mode, and SHALL follow each `current_mode_update` to its new mode. Before that, it SHALL show no mode.

#### Scenario: The mode at start
- **WHEN** the session becomes ready with current mode `auto`
- **THEN** the session buffer's mode line contains `[auto]`

#### Scenario: A mode change
- **WHEN** the agent sends `current_mode_update` with `plan`
- **THEN** the session buffer's mode line contains `[plan]` and no longer `[auto]`

#### Scenario: Not ready
- **WHEN** the agent has not answered `session/new`
- **THEN** the session buffer's mode line shows no mode in brackets
