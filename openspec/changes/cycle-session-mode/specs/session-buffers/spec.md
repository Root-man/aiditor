## ADDED Requirements

### Requirement: S-TAB cycles the session's mode
`S-TAB` in a session buffer (`agent-cycle-mode`) SHALL send `session/set_config_option` setting the session's mode option to the value after its current one in the option's order (groups flattened), wrapping from the last value to the first. The values and current value are those of the latest `session/new` answer, `config_option_update` or `session/set_config_option` answer. It SHALL work while a turn runs. The mode line SHALL follow the answer. A session without a mode option SHALL echo `<agent> has no modes`; before `session/new` is answered it SHALL echo `<agent> is not ready`, and after the agent exits `<agent> session has ended`; in these cases nothing is sent. If the agent answers with an error, `*Messages*` SHALL get `<agent> could not set mode <value>: <reason>` with the value sent, and the mode line keeps its mode.

#### Scenario: Cycling through every mode
- **WHEN** the session's mode option is `auto` with values `auto` and `plan`, and `S-TAB` is typed, the agent answers, and `S-TAB` is typed again and answered
- **THEN** the first sends `plan` and the mode line shows `[plan]`, the second sends `auto` and the mode line shows `[auto]`

#### Scenario: Following the agent's own change
- **WHEN** the agent sends `config_option_update` setting the mode to `plan`, then `S-TAB` is typed
- **THEN** `auto` is sent

#### Scenario: No mode option
- **WHEN** the session has no mode option and `S-TAB` is typed
- **THEN** `claude has no modes` is echoed and nothing is sent

#### Scenario: Not ready or ended
- **WHEN** `S-TAB` is typed before `session/new` is answered, or after the agent exited
- **THEN** `claude is not ready`, or `claude session has ended`, is echoed and nothing is sent

#### Scenario: A refused cycle
- **WHEN** the mode is `plan` and the agent answers the cycle to `auto` with an error
- **THEN** `*Messages*` has `claude could not set mode auto: ` followed by the reason and the mode line keeps `[plan]`
