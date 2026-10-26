## ADDED Requirements

### Requirement: Replaying a transcript
With `AIDITOR_FAKE_AGENT_REPLAY` naming a transcript in the recorder's format, relative to its working directory, the fake agent SHALL replay it instead of running a script: it SHALL write the agent messages recorded before the next client message, then read one client message and check it against that recorded one, and repeat. A client request or notification SHALL match a recorded one with the same method; a client response SHALL match a recorded response with the same id. Recorded agent responses SHALL carry the id of the live client request their recorded request matched. Every `{cwd}` in an agent message SHALL be written as the `cwd` of the live `session/new`. A client message that doesn't match, or that arrives after the transcript's last client message, SHALL make the fake write `unexpected client message: <the message>` and what the transcript expected to stderr and exit with code 1. A transcript that cannot be read or parsed SHALL make it write the reason to stderr and exit with code 2. When its stdin closes it SHALL exit with code 0.

#### Scenario: A recorded session replays
- **WHEN** the agent service spawns the fake with a committed transcript and the client sends what was recorded
- **THEN** the session opens, each prompt's recorded updates arrive and its answer carries the recorded stop reason

#### Scenario: An unexpected client message
- **WHEN** the client sends a message the transcript doesn't have next, such as a `session/cancel`
- **THEN** the fake writes `unexpected client message:` with the message to stderr and exits with code 1
