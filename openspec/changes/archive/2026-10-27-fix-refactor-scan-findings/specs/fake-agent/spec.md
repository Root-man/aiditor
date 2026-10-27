## MODIFIED Requirements

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
