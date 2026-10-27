# fake-agent Specification

## Purpose
A scripted ACP agent for tests: a JSON script file decides what it sends and does after a session opens and on each prompt, so agent scenarios need no new code in the fake.

## Requirements

### Requirement: The script names what the fake does
The fake agent SHALL read the JSON script file named by `AIDITOR_FAKE_AGENT`, relative to its working directory. The script is an object with an optional `session` list of steps, run once after the fake answers `session/new`, and an optional `prompts` list of step lists, where the n-th `session/prompt` runs the n-th list. Without `AIDITOR_FAKE_AGENT` the script is empty. A script that cannot be read or parsed SHALL make the fake write the reason to stderr and exit with code 2 before speaking ACP. On every start the fake SHALL report on stderr its pid, the file system and terminal capabilities the client advertised, the session's `cwd` and the variable `A`, and SHALL answer `session/new` with session id `fake-session` and one config option: a select with id `mode`, category `mode`, values `auto` and `plan`, current value `auto`.

#### Scenario: No script
- **WHEN** the fake is spawned without `AIDITOR_FAKE_AGENT` and opens a session
- **THEN** the session opens with id `fake-session` and a `mode` option with values `auto` and `plan`, current `auto`

#### Scenario: A bad script
- **WHEN** `AIDITOR_FAKE_AGENT` names a file that is not a valid script
- **THEN** the fake writes the reason to stderr and exits with code 2

### Requirement: Steps
A step SHALL be one of:
- `{"update": <SessionUpdate>}`: send a `session/update` for the session with that update, written as ACP JSON;
- `{"permission": {"toolCall": <ToolCallUpdate>, "options": [<PermissionOption>]}}`: send `session/request_permission` and wait for the answer, then write `permission answered: <outcome JSON>` or `permission error: <message>` to stderr;
- `{"write": {"path": <path>, "text": <text>}}`: write the text to the path relative to the session's `cwd`, creating missing parent directories, or write `write error: <reason>` to stderr;
- `{"sleep": <ms>}`: wait that many milliseconds;
- `{"stderr": <line>}`: write the line to stderr;
- `{"exit": <code>}`: exit the process at once with the code;
- `{"stop": <StopReason>}`: end the turn with that stop reason, skipping the turn's remaining steps;
- `"ignoreSigterm"`: ignore SIGTERM from then on;
- `"closeStdout"`: close the process's stdout, so the client hears nothing more, and keep running the remaining steps.

Steps SHALL run in order, each after the previous one finished. A turn that runs out of steps, or a prompt past the end of `prompts`, SHALL end with `end_turn`. A `stop` in the `session` steps ends them.

#### Scenario: A turn streams, asks, writes and ends
- **WHEN** a prompt's steps send an agent message chunk, ask a permission, write `out/a.txt` and stop with `max_tokens`, and the client answers the permission with an option
- **THEN** the client gets the update, then the permission request, the file exists under `cwd` with the text after the answer, and the prompt is answered with `max_tokens`

#### Scenario: Prompts past the script
- **WHEN** the client sends more prompts than the script has
- **THEN** each extra prompt is answered with `end_turn`

#### Scenario: Closing stdout keeps the process alive
- **WHEN** the fake's session steps are `"closeStdout"`, `{"sleep": 200}` and `{"stderr": "still here"}`
- **THEN** the client's connection closes, and `still here` is written to stderr afterwards

### Requirement: Modes and cancel
The fake SHALL answer `session/set_config_option` for its `mode` option and a value it advertised with its config options, the mode set to that value, and then send a `config_option_update` with the same options; for any other option or value it SHALL answer with an invalid-params error. On `session/cancel` it SHALL stop the running turn at once, including a step that is sleeping or waiting for a permission answer, and answer that prompt with `cancelled`.

#### Scenario: Setting the mode
- **WHEN** the client sets the `mode` option to `plan`
- **THEN** the answer and a following `config_option_update` both have the `mode` option's current value `plan`

#### Scenario: An unknown value
- **WHEN** the client sets the `mode` option to `nope`
- **THEN** the request fails with an invalid-params error

#### Scenario: Cancelling a turn
- **WHEN** the client cancels a turn whose next step sleeps for a minute
- **THEN** the prompt is answered with `cancelled` within seconds

### Requirement: Replaying a transcript
With `AIDITOR_FAKE_AGENT_REPLAY` naming a transcript in the recorder's format, relative to its working directory, the fake agent SHALL replay it instead of running a script: it SHALL write the agent messages recorded before the next client message, then read one client message and check it against that recorded one, and repeat. A client request or notification SHALL match a recorded one with the same method; a client response SHALL match a recorded response with the same id. Recorded agent responses SHALL carry the id of the live client request their recorded request matched. Every `{cwd}` in an agent message SHALL be written as the `cwd` of the live `session/new`. A client message that doesn't match, or that arrives after the transcript's last client message, SHALL make the fake write `unexpected client message: <the message>` and what the transcript expected to stderr and exit with code 1. A transcript that cannot be read or parsed SHALL make it write the reason to stderr and exit with code 2. When its stdin closes it SHALL exit with code 0.

#### Scenario: A recorded session replays
- **WHEN** the agent service spawns the fake with a committed transcript and the client sends what was recorded
- **THEN** the session opens, each prompt's recorded updates arrive and its answer carries the recorded stop reason

#### Scenario: An unexpected client message
- **WHEN** the client sends a message the transcript doesn't have next, such as a `session/cancel`
- **THEN** the fake writes `unexpected client message:` with the message to stderr and exits with code 1
