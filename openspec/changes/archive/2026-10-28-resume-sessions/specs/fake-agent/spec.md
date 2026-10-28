## ADDED Requirements

### Requirement: Loading a session
The script MAY have a `history` list of ACP session updates. With it, the fake SHALL advertise `loadSession`; without it, it SHALL NOT. On `session/load` the fake SHALL report the session id and `cwd` on stderr as `load=<id>` and `cwd=<cwd>`, send each history update in order for the requested session id, and then answer with the same config option as `session/new`. Prompts after a load SHALL run the script's `prompts` as after `session/new`.

#### Scenario: A load replays the history
- **WHEN** the script's history is a user message chunk and an agent message chunk and the client loads session `old`
- **THEN** the fake sends both updates for `old` before answering, and its stderr has `load=old`
