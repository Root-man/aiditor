# Spec Delta

## ADDED Requirements

### Requirement: The frame display carries the echo area message
The frame display SHALL include the echo area's text: the current message, or none when the echo area is blank.

#### Scenario: A command error in a frame snapshot
- **WHEN** a command fails with `boom`
- **THEN** the frame display's message is `boom`

#### Scenario: No message
- **WHEN** a buffer is shown and nothing has been reported
- **THEN** the frame display has no message
