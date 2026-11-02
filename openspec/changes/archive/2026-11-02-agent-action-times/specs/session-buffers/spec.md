# Spec Delta

## ADDED Requirements

### Requirement: Messages show their clock time
A user or agent message that arrives live SHALL show the local `HH:MM` of its arrival right-aligned on its first line, in the `annotation` face. An agent message SHALL show it when its block ends; a sent prompt SHALL show it at once.

#### Scenario: A sent prompt shows its time
- **WHEN** the clock reads `14:01` and `hello RET` is typed
- **THEN** the echo's first line ends with `14:01` in `annotation`, right-aligned

#### Scenario: A reply shows the time its first chunk arrived
- **WHEN** a message's first chunk arrives at `14:03`, the clock passes `14:04` and the turn ends
- **THEN** the message's first line ends with `14:03`

### Requirement: Thoughts show how long they took
A live thought's block SHALL, when it ends, start with the line `Thought for <n>s`, the whole seconds from its first chunk to its end, in the thought's face.

#### Scenario: A thought's header
- **WHEN** a thought's first chunk arrives and 4.2s later a message chunk arrives
- **THEN** the thought's block starts with `Thought for 4s`

### Requirement: Tool calls show how long they ran
When a live tool call first reaches `completed` or `failed`, its header SHALL show its duration right-aligned in `annotation`: the time since it arrived less the time it waited for permission, as `0.3s` under a minute and `2m 05s` from a minute on.

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
While a turn runs, the last block above the prompt line SHALL be the line `Working for <duration>` in `session-thought`, counted from sending the prompt, permission waits included, ticking each second; `<duration>` reads `12s`, `2m 05s` or `1h 02m 05s`. Output SHALL go in above it. When the turn ends, by the answer to `session/prompt`, a cancel or the agent exiting, the same line SHALL become `Worked <duration>` with the clock time right-aligned in `annotation`, and stay where it is.

#### Scenario: It ticks above the prompt
- **WHEN** a turn has run for 136s and the session is shown
- **THEN** the line above the prompt line reads `Working for 2m 16s`, and output arriving goes in above it

#### Scenario: The line settles however the turn ends
- **WHEN** a turn ends with `end_turn`, with `cancelled`, with an error answer or by the agent exiting
- **THEN** the line reads `Worked <duration>` in the same place, under any `Turn ended` or `Error` line

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
