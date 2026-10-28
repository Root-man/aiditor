## MODIFIED Requirements

### Requirement: A session starts in the agent's default mode
When a new session opens (`session/new` answers) and the agent's registry entry has a `default_mode`, the core SHALL send `session/set_config_option` setting the mode option to it if it is one of the option's values and not its current value, and SHALL then show the mode the answer reports. If `default_mode` is not one of the values, or the agent has no mode option, `*Messages*` SHALL get `<agent> has no mode <default_mode>` and nothing is sent. If the agent answers with an error, `*Messages*` SHALL get `<agent> could not set mode <default_mode>: <reason>`. In both cases the mode stays as the agent reported it. A loaded session SHALL keep the mode its `session/load` answer reports, with nothing sent and no note.

#### Scenario: Starting in the default mode
- **WHEN** an agent with `default_mode = "plan"` opens a session whose mode option is `auto` with values `auto` and `plan`
- **THEN** the core sends `session/set_config_option` for the option with `plan`, and after the answer the mode line shows `[plan]`

#### Scenario: Already in the default mode
- **WHEN** an agent with `default_mode = "auto"` opens a session whose mode option is already `auto`
- **THEN** nothing is sent

#### Scenario: An unknown default mode
- **WHEN** an agent with `default_mode = "nope"` opens a session whose mode values are `auto` and `plan`
- **THEN** `*Messages*` has `claude has no mode nope`, nothing is sent and the mode line shows `[auto]`

#### Scenario: The agent refuses the mode
- **WHEN** the agent answers the `session/set_config_option` with an error
- **THEN** `*Messages*` has `claude could not set mode plan: ` followed by the reason and the mode line keeps its mode

#### Scenario: A loaded session keeps its mode
- **WHEN** an agent with `default_mode = "plan"` loads a session whose mode option is `auto`
- **THEN** nothing is sent and the mode line shows `[auto]`
