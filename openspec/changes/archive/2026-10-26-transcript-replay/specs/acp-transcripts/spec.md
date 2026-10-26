## ADDED Requirements

### Requirement: Committed transcripts replay against the client
A test SHALL replay every committed transcript through the fake agent's replay mode, the agent service and the core: it SHALL start the session with `agent-start` from a registry entry and type each recorded prompt into the session buffer, and SHALL assert that every recorded `session/update` reaches the core as a typed update, every recorded agent request reaches the core and is answered, and every turn ends with its recorded stop reason. When the fake exits or a turn fails, the test SHALL fail showing the fake's stderr.

#### Scenario: Every transcript replays
- **WHEN** `cargo test` runs on macOS or Linux
- **THEN** each committed transcript replays to its last message and the assertions hold

#### Scenario: An unexpected message fails the replay
- **WHEN** the replay sends a message the transcript doesn't expect
- **THEN** the test fails with the message shown
