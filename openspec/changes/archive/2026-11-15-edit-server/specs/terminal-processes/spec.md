# Spec Delta

## ADDED Requirements

### Requirement: The edit client in the terminal's environment
When the editor has a socket, each terminal's process SHALL get `AIDITOR_SOCKET` set to the socket's path, `AIDITOR_TERMINAL` set to its terminal id, and `GIT_EDITOR`, `EDITOR` and `VISUAL` set to the editor's executable, shell-quoted, followed by ` --wait`. These SHALL override the same variables from the editor's environment and from the spawn's extra variables.

#### Scenario: EDITOR is the client
- **WHEN** the editor has a socket and a terminal runs `echo $EDITOR`
- **THEN** the output is the editor's executable path followed by ` --wait`

#### Scenario: An inherited GIT_EDITOR is replaced
- **WHEN** the editor's environment has `GIT_EDITOR=true` and a terminal runs `echo $GIT_EDITOR`
- **THEN** the output is the client command, not `true`

#### Scenario: No socket
- **WHEN** the editor has no socket
- **THEN** a terminal gets no `AIDITOR_SOCKET`, and `EDITOR` is left as inherited
