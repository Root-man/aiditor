## MODIFIED Requirements

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

### Requirement: The working line
While a turn runs, the last block above the prompt line SHALL be the line `Working for <duration>` in `session-thought`, counted from sending the prompt, permission waits included, ticking each second; `<duration>` reads `12s`, `2m 05s` or `1h 02m 05s`. Output SHALL go in above it. When the turn ends, by the answer to `session/prompt`, a cancel or the agent exiting, the same line SHALL become `Worked <duration>` with the clock time right-aligned, both in `annotation`, and stay where it is. The settled line SHALL be drawn as a rule: indented two columns, with a 1 px line in the `separator` foreground through the middle of the row from the window's left edge to a column before the label, from a column after the label to a column before the time, and from a column after the time to the window's right edge.

#### Scenario: It ticks above the prompt
- **WHEN** a turn has run for 136s and the session is shown
- **THEN** the line above the prompt line reads `Working for 2m 16s` with no rule, and output arriving goes in above it

#### Scenario: The line settles however the turn ends
- **WHEN** a turn ends with `end_turn`, with `cancelled`, with an error answer or by the agent exiting
- **THEN** the line reads `Worked <duration>` and its time in the same place, under any `Turn ended` or `Error` line, drawn as a rule

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
