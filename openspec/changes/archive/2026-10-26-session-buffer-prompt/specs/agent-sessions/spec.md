# Spec Delta

## MODIFIED Requirements

### Requirement: The spawn waits for the environment capture
When the project's environment is being captured as the session starts, the spawn SHALL wait and SHALL be requested once that capture answers, with the captured variables, or with none when it failed. A session whose session buffer is killed while it waits SHALL never be spawned.

#### Scenario: Waiting for the capture
- **WHEN** the editor was launched from the Dock, the project's capture is running and `claude` is started
- **THEN** no spawn is requested until the capture answers with `PATH=/opt/bin`, and then the spawn carries `PATH=/opt/bin`

#### Scenario: Killed while waiting
- **WHEN** a session waits for the capture and its session buffer is killed before the capture answers
- **THEN** no spawn is requested when the capture answers

### Requirement: The stderr log buffer
Each session SHALL get a new read-only buffer named `*<agent> stderr*<<workspace>>`, made unique, owned by the current workspace and not shown. Each stderr line of the agent SHALL be appended to it as a line while it exists. The buffer SHALL stay after the agent exits, and killing it SHALL neither ask nor stop the agent.

#### Scenario: Stderr is logged
- **WHEN** a started agent writes `hello` then `world` to stderr
- **THEN** its log buffer's text is `hello\nworld\n` and the buffer is read-only

#### Scenario: Killing the log buffer
- **WHEN** a live session's log buffer is killed
- **THEN** it is killed without a question and no kill is sent to the agent

## REMOVED Requirements

### Requirement: Killing the log buffer stops the agent
**Reason**: The session buffer replaces the log buffer as the session's handle.
**Migration**: Kill the session buffer, or its workspace, to stop the agent.

## ADDED Requirements

### Requirement: Killing the session buffer stops the agent
Killing a live session's session buffer SHALL first ask `Buffer <name> has a running process; kill it? `, completing over `no` then `yes`, as for a live terminal; on `yes` the buffer SHALL be killed and the agent stopped, with no crash note. An ended session's buffer SHALL be killed without asking. Killing a workspace SHALL stop the agents of the session buffers it kills.

#### Scenario: Killing the session buffer asks
- **WHEN** `C-x k` names a live session's buffer and `yes` is answered
- **THEN** the minibuffer first asked `Buffer <name> has a running process; kill it? `, and then a kill is sent to the agent

#### Scenario: Killing a workspace stops its agent
- **WHEN** a workspace with a live session is killed
- **THEN** a kill is sent to that session's agent and `*Messages*` gets no crash note when it exits
