# Spec Delta

## ADDED Requirements

### Requirement: A panicking command is contained
When a command handler panics, the core SHALL catch the panic and keep running. The message line SHALL name the command and say that its state may be inconsistent. `*Messages*` SHALL get that line plus the panic message, where it happened and a backtrace. Later keys and commands SHALL work as before.

#### Scenario: A test command panics
- **WHEN** a test registers a command whose handler panics with `boom` and calls it
- **THEN** the call returns, the message line names the command and says its state may be inconsistent, and `*Messages*` holds `boom`, the panic's source location and a backtrace

#### Scenario: The editor keeps working after a panic
- **WHEN** a command has panicked and the user then types `a`
- **THEN** `a` is inserted in the current buffer

### Requirement: Builds that abort on panic are rejected
A build of the core with panics set to abort SHALL fail, since aborting would silently disable panic containment.

#### Scenario: A profile sets panic to abort
- **WHEN** any build profile sets `panic = "abort"` and the workspace is built, as CI does
- **THEN** the build fails with an error saying panics must unwind
