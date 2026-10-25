# Spec Delta

## Purpose

A scripted ACP agent for tests: a JSON script file decides what it sends and does after a session opens and on each prompt, so agent scenarios need no new code in the fake.

## ADDED Requirements

### Requirement: The script names what the fake does
The fake agent SHALL read the JSON script file named by `AIDITOR_FAKE_AGENT`, relative to its working directory. The script is an object with an optional `session` list of steps, run once after the fake answers `session/new`, and an optional `prompts` list of step lists, where the n-th `session/prompt` runs the n-th list. Without `AIDITOR_FAKE_AGENT` the script is empty. A script that cannot be read or parsed SHALL make the fake write the reason to stderr and exit with code 2 before speaking ACP. On every start the fake SHALL report on stderr its pid, the file system and terminal capabilities the client advertised, the session's `cwd` and the variable `A`, and SHALL answer `session/new` with session id `fake-session` and modes `auto` (current) and `plan`.

#### Scenario: No script
- **WHEN** the fake is spawned without `AIDITOR_FAKE_AGENT` and opens a session
- **THEN** the session opens with id `fake-session` and modes `auto` and `plan`

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
- `"ignoreSigterm"`: ignore SIGTERM from then on.

Steps SHALL run in order, each after the previous one finished. A turn that runs out of steps, or a prompt past the end of `prompts`, SHALL end with `end_turn`. A `stop` in the `session` steps ends them.

#### Scenario: A turn streams, asks, writes and ends
- **WHEN** a prompt's steps send an agent message chunk, ask a permission, write `out/a.txt` and stop with `max_tokens`, and the client answers the permission with an option
- **THEN** the client gets the update, then the permission request, the file exists under `cwd` with the text after the answer, and the prompt is answered with `max_tokens`

#### Scenario: Prompts past the script
- **WHEN** the client sends more prompts than the script has
- **THEN** each extra prompt is answered with `end_turn`

### Requirement: Modes and cancel
The fake SHALL answer `session/set_mode` for a mode it advertised and then send a `current_mode_update` with that mode; for any other mode it SHALL answer with an invalid-params error. On `session/cancel` it SHALL stop the running turn at once, including a step that is sleeping or waiting for a permission answer, and answer that prompt with `cancelled`.

#### Scenario: Setting the mode
- **WHEN** the client sets mode `plan`
- **THEN** the request succeeds and the client gets a `current_mode_update` with `plan`

#### Scenario: Cancelling a turn
- **WHEN** the client cancels a turn whose next step sleeps for a minute
- **THEN** the prompt is answered with `cancelled` within seconds
