# Spec Delta

## MODIFIED Requirements

### Requirement: Quitting asks before killing live terminals
When any terminal's shell has not exited, or any agent session is live (waiting for its environment, starting or ready), `save-buffers-kill-terminal` SHALL ask `Active processes exist; kill them and exit anyway? `, completing over `No` then `Yes`, after the modified-buffer questions and before starting the saves they chose. Only `Yes` SHALL go on to quit; any other answer SHALL keep the editor open with its terminals and agents running, still saving the buffers answered `Yes`.

#### Scenario: No keeps the terminal running
- **WHEN** a terminal is live and `C-x C-c RET` is typed (the default `No`)
- **THEN** the app does not quit and no `TerminalOp::Kill` is sent

#### Scenario: Yes quits
- **WHEN** a terminal is live and `C-x C-c yes RET` is typed
- **THEN** the app quits

#### Scenario: After the save questions
- **WHEN** a file buffer is modified, a terminal is live and `C-x C-c RET` is typed
- **THEN** the next prompt is `Active processes exist; kill them and exit anyway? `, and `yes RET` saves the file and then quits

#### Scenario: A live agent session asks too
- **WHEN** an agent session is ready, no terminal is live and `C-x C-c RET` is typed
- **THEN** the app does not quit and no `AgentOp::Kill` is sent
