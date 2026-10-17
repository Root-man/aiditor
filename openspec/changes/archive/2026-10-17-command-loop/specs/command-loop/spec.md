# Spec Delta

## Purpose

The command loop turns key sequences into command runs and remembers what ran and which keys came in, so repeat-aware commands and keyboard macros build on it.

## ADDED Requirements

### Requirement: Key sequences run their commands
A key sequence fed to the core SHALL run the command it is bound to, and that command's edits SHALL show in the current buffer.

#### Scenario: A bound test command edits the buffer
- **WHEN** a test command that inserts `hi` is bound to `C-c t` and the keys `C-c t` are fed to the core
- **THEN** the current buffer's text is `hi`

### Requirement: The loop tracks this and last command
While a key sequence's command runs, the core SHALL report it as this command and the previous sequence's command as last command. After it, that command SHALL be the last command. A prefix key SHALL change neither. A sequence that runs no command SHALL leave no last command. `C-g` quitting a prefix SHALL count as `keyboard-quit`. A command called by name from inside another SHALL change neither.

#### Scenario: A command sees what ran before it
- **WHEN** a test command bound to `C-c t` is run twice by keys
- **THEN** on the first run it sees no last command, on the second it sees itself, and both times this command is its own name

#### Scenario: A prefix keeps the last command
- **WHEN** `C-c t` runs and then `C-x` is pressed
- **THEN** the last command is still the test command

#### Scenario: Typing clears the last command
- **WHEN** `C-c t` runs and then `a` is typed
- **THEN** there is no last command

#### Scenario: Quitting a prefix
- **WHEN** `C-x C-g` is pressed
- **THEN** the last command is `keyboard-quit`

#### Scenario: Nested calls leave tracking alone
- **WHEN** a command bound to `C-c n` calls `undo` by name
- **THEN** the last command is the bound command, not `undo`

### Requirement: The key stream can be recorded
The core SHALL start recording on request; from then every key it receives SHALL be kept in order, prefix and quit keys included, until the recorded keys are taken, which SHALL stop recording. Taking keys when not recording SHALL return nothing.

#### Scenario: Recorded keys come back in order
- **WHEN** recording starts and `a C-x C-g b` is fed
- **THEN** taking the recorded keys returns `a C-x C-g b`, and keys fed afterwards are not kept

#### Scenario: Not recording
- **WHEN** keys are fed without recording having started
- **THEN** taking the recorded keys returns nothing
