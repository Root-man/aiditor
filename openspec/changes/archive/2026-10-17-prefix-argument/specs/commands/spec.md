## MODIFIED Requirements

### Requirement: Every command receives the prefix argument
Every call SHALL carry the prefix argument, whatever the command's argument spec says. It SHALL be absent, or one of: given by `C-u` with a value, a lone minus worth -1, or a signed number; each form SHALL have a numeric value.

#### Scenario: No prefix argument
- **WHEN** a command is called without a prefix argument
- **THEN** the handler sees it as absent

#### Scenario: Raw form and value
- **WHEN** a command is called with a `C-u` prefix argument of 4
- **THEN** the handler can tell it from the number 4, and both have the value 4
