# Spec Delta

## ADDED Requirements

### Requirement: The core gets the wall clock from the frontend
The frontend SHALL give the core the workspace clock's `HH:MM` text at startup and at each minute wakeup, so the core never reads the wall clock.

#### Scenario: A minute passes
- **WHEN** the workspace clock changes from `20:41` to `20:42`
- **THEN** a message stamped afterwards shows `20:42`
