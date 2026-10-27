# acp-transcripts Specification

## Purpose
A transparent recorder for ACP traffic, and scrubbed transcripts of real Claude Code and Codex sessions recorded with it, for tests that replay what the adapters send.

## Requirements

### Requirement: The recorder passes traffic through and records it
`aiditor-acp-record OUT -- COMMAND [ARGS...]` SHALL run COMMAND with ARGS, its stderr inherited, copy its own stdin to the command's stdin and the command's stdout to its own stdout byte for byte, and append each line in either direction to OUT as one JSON line `{"from": "client" | "agent", "message": <the line as JSON>}`; a line that is not JSON SHALL be recorded as a JSON string. OUT SHALL be created if missing and appended to otherwise, each line written as it passes. When its stdin closes it SHALL close the command's stdin; when the command exits it SHALL exit with the command's code, or 128 plus the signal that killed it. Wrong usage or a command that cannot be started SHALL print the reason to stderr and exit with code 2.

#### Scenario: A session through the recorder
- **WHEN** aiditor's agent service spawns the recorder in front of the fake agent and the session opens and a prompt is answered
- **THEN** the session works as without the recorder, and OUT holds the `initialize` request from the client, its answer from the agent, and the prompt and its answer, each with its direction

#### Scenario: Bad usage
- **WHEN** the recorder is run without `--` or without a command
- **THEN** it prints its usage to stderr and exits with code 2

### Requirement: Committed transcripts
`crates/aiditor-services/tests/transcripts/` SHALL hold a Claude Code and a Codex transcript in the recorder's format, recorded through aiditor's agent service with its client capabilities, each covering a markdown reply, a tool call that reads a file, one that edits a file and a permission request (for the edit or another tool call), plus a mode change where the agent makes one on its own (Claude Code's plan mode; Codex makes none). In committed transcripts the session's `cwd` SHALL read `{cwd}`, the home directory `{home}` and every email address `user@example.com`, each `available_commands_update` SHALL list at most 3 commands, and no tokens or other private paths SHALL remain. A README next to them SHALL say how to record and scrub them again.

#### Scenario: Committed transcripts are scrubbed
- **WHEN** the test suite reads every committed transcript
- **THEN** each line parses as a recorder line, no line or joined streamed text contains the home directory, `/Users/` or `/home/`, no email but `user@example.com` is left, and no command list is longer than 3

### Requirement: Committed transcripts replay against the client
A test SHALL replay every committed transcript through the fake agent's replay mode, the agent service and the core: it SHALL start the session with `agent-start` from a registry entry and type each recorded prompt into the session buffer, answer each permission request by typing `C-c C-y` in the session buffer, and SHALL assert that every recorded `session/update` reaches the core as a typed update, every recorded agent request reaches the core and is answered, the session buffer shows an answer line for each recorded permission request, and every turn ends with its recorded stop reason. When the fake exits or a turn fails, the test SHALL fail showing the fake's stderr.

#### Scenario: Every transcript replays
- **WHEN** `cargo test` runs on macOS or Linux
- **THEN** each committed transcript replays to its last message and the assertions hold

#### Scenario: An unexpected message fails the replay
- **WHEN** the replay sends a message the transcript doesn't expect
- **THEN** the test fails with the message shown
