## ADDED Requirements

### Requirement: Config options reach the agent
A set-config-option op SHALL send its `session/set_config_option` request to the agent once its session is open, and the core SHALL get a message for the agent id with the config options the agent answered, or the error's message. It SHALL be ignored for an unknown agent or one whose session isn't open.

#### Scenario: Setting the mode option
- **WHEN** a set-config-option op sets a fake agent's `mode` option to `plan`
- **THEN** the core gets the answer whose `mode` option's current value is `plan`

#### Scenario: A refused value
- **WHEN** a set-config-option op sets a fake agent's `mode` option to `nope`
- **THEN** the core gets an error answer
