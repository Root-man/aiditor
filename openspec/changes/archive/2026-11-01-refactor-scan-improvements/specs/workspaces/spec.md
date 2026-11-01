# Spec Delta

## ADDED Requirements

### Requirement: Killing a workspace asks about its live processes
After the save questions, when a buffer of that workspace is a live terminal or a session that hasn't ended, both kill commands SHALL ask `Active processes exist; kill them and the workspace anyway? ` over `No` then `Yes`. `Yes` SHALL kill the workspace; any other answer SHALL leave it, still saving the buffers answered `Yes`.

#### Scenario: A live terminal
- **WHEN** workspace `b` holds a live terminal and `C-x t 0 yes RET` is typed
- **THEN** the prompt was `Active processes exist; kill them and the workspace anyway? ` and `b` is killed

#### Scenario: Declining keeps the workspace
- **WHEN** workspace `b` holds a live session and the question is answered `No`
- **THEN** `b`, its buffer and the agent remain

#### Scenario: Saves still happen after declining
- **WHEN** `b` has a modified file buffer and a live terminal, the save question is answered `Yes` and the process question `No`
- **THEN** the file is written and `b` remains

#### Scenario: Processes of other workspaces are not asked about
- **WHEN** only workspace `a` holds a live terminal and `b` is killed
- **THEN** no process question is asked

#### Scenario: An ended session
- **WHEN** `b` holds only a session whose agent has exited and `b` is killed
- **THEN** no process question is asked
