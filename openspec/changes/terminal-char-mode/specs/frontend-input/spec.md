## ADDED Requirements

### Requirement: Navigation and function keys reach the core
Home, End, PageUp, PageDown, Delete and F1 to F12 SHALL be sent to the core as keys with their modifiers.

#### Scenario: F1 and Home
- **WHEN** `f1` and `shift-home` are pressed
- **THEN** the core receives `<f1>` and `S-<home>`
