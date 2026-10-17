# commands Specification

## Purpose

Named commands that keys and `M-x` both call, each declaring the arguments it needs, so every feature is reachable by name and testable without keys.

## Requirements

### Requirement: Commands are looked up by name
The core SHALL hold a registry of commands by name. Calling a name with arguments SHALL run that command's handler with them. Calling a name that is not registered SHALL fail with a message naming it.

#### Scenario: A registered command runs with its arguments
- **WHEN** a test registers a command and calls it by name with the prefix argument 3 and the string `hello`
- **THEN** the handler runs and sees the prefix argument 3 and the string `hello`

#### Scenario: An unknown name is reported
- **WHEN** a test calls the name `no-such-command`
- **THEN** no handler runs and the message line shows an error naming `no-such-command`

### Requirement: Every command receives the prefix argument
Every call SHALL carry the prefix argument, whatever the command's argument spec says. It SHALL be absent, or one of: given by `C-u` with a value, a lone minus worth -1, or a signed number; each form SHALL have a numeric value.

#### Scenario: No prefix argument
- **WHEN** a command is called without a prefix argument
- **THEN** the handler sees it as absent

#### Scenario: Raw form and value
- **WHEN** a command is called with a `C-u` prefix argument of 4
- **THEN** the handler can tell it from the number 4, and both have the value 4

### Requirement: A command that needs the region gets one or fails
A command whose argument spec asks for the region SHALL receive a char range. When the caller gives none, the range SHALL run between point and mark of the current window, lower end first. When neither is available because no mark is set, the call SHALL fail without running the handler.

#### Scenario: Region from point and mark
- **WHEN** the mark is at 1, point is at 4 and a region command is called without a region
- **THEN** the handler gets the range 1 to 4

#### Scenario: Region given by the caller
- **WHEN** a region command is called with the range 0 to 2
- **THEN** the handler gets the range 0 to 2, whatever point and mark are

#### Scenario: No mark
- **WHEN** no mark is set and a region command is called without a region
- **THEN** the handler does not run and the message line says the mark is not set

### Requirement: Prompted strings must be supplied
A command whose argument spec lists prompts SHALL receive one string per prompt. A call with fewer strings than prompts SHALL fail without running the handler.

#### Scenario: Missing string
- **WHEN** a command with one prompt is called with no strings
- **THEN** the handler does not run and the message line shows an error naming the prompt

### Requirement: Command errors go to the message line
An error a handler returns SHALL be shown on the message line, and the editor SHALL keep running.

#### Scenario: Handler error
- **WHEN** a command's handler fails with `boom`
- **THEN** the message line shows `boom`

### Requirement: Built-in commands are callable by name
Undo, redo, saving the current buffer and reloading settings SHALL be registered as `undo`, `undo-redo`, `save-buffer` and `reload-settings`, and their existing keys SHALL run them with unchanged behavior.

#### Scenario: Undo by name
- **WHEN** `ab` is typed and `undo` is called by name
- **THEN** the buffer is empty
