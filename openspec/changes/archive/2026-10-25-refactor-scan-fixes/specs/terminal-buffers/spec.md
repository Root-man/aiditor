# Spec Delta

## MODIFIED Requirements

### Requirement: The buffer follows the shell
When the shell exits, its buffer SHALL be killed. When the spawn fails, the buffer SHALL be killed and the reason shown in the echo area. In both cases the core SHALL send the terminal one kill, so the service releases its PTY and input thread at once rather than at quit. When a terminal buffer is killed by any path, its shell SHALL be killed.

#### Scenario: exit
- **WHEN** the terminal service reports a terminal's shell exited
- **THEN** its buffer is killed and the core sends exactly one `TerminalOp::Kill` for that terminal, and no other op

#### Scenario: Spawn failure
- **WHEN** the terminal service reports a spawn failed with `/x: not a directory`
- **THEN** the buffer is killed, the echo area shows `/x: not a directory`, and the core sends exactly one `TerminalOp::Kill` for that terminal

#### Scenario: Killing a workspace
- **WHEN** a workspace holding two terminals is killed
- **THEN** the core sends `TerminalOp::Kill` for both
