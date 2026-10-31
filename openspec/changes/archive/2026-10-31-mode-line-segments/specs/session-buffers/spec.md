# Spec Delta

## MODIFIED Requirements

### Requirement: The mode line shows the current mode
A session buffer's mode line SHALL show `[<value>]` as a left group segment after the buffer name once the session is ready and has a mode option (the first config option of category `mode`), its value being the option's current value. It SHALL follow each `config_option_update` and each `session/set_config_option` answer to the mode option's new current value, and show no mode when they no longer have a mode option. Before the session is ready, it SHALL show no mode.

#### Scenario: The mode at start
- **WHEN** the session becomes ready with a mode option whose current value is `auto`
- **THEN** the session buffer's mode line contains `[auto]`

#### Scenario: A mode change
- **WHEN** the agent sends `config_option_update` whose mode option's current value is `plan`
- **THEN** the session buffer's mode line contains `[plan]` and no longer `[auto]`

#### Scenario: Not ready
- **WHEN** the agent has not answered `session/new`
- **THEN** the session buffer's mode line shows no mode in brackets
