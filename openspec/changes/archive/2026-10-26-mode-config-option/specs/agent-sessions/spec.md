## MODIFIED Requirements

### Requirement: Session notes in Messages
When the agent's session opens, `*Messages*` and the echo area, replacing `Starting <agent>...`, SHALL get `<agent> session ready (modes: <ids>)`: the values of the session's mode option (the first config option in the `session/new` answer with category `mode`, its groups flattened) joined by `, `, or `none` when it has no mode option. When the agent fails to start, `*Messages*` and the echo area SHALL get `<agent> session failed to start: <reason>`. When the agent exits after its session opened and was not stopped by the editor, `*Messages*` SHALL get `<agent> session exited with code <code>`, or `<agent> session killed by <signal>`.

#### Scenario: Ready with modes
- **WHEN** the agent answers `session/new` with a `mode` option whose values are `auto` and `plan`
- **THEN** `*Messages*` has `claude session ready (modes: auto, plan)` and the echo area shows it

#### Scenario: Ready without a mode option
- **WHEN** the agent answers `session/new` with no config option of category `mode`
- **THEN** `*Messages*` has `claude session ready (modes: none)`

#### Scenario: A failed start
- **WHEN** the agent's command cannot be found
- **THEN** `*Messages*` has `claude session failed to start: ` followed by the reason

#### Scenario: A crash
- **WHEN** a ready agent exits with code 3
- **THEN** `*Messages*` has `claude session exited with code 3`

## ADDED Requirements

### Requirement: A session starts in the agent's default mode
When the session opens and the agent's registry entry has a `default_mode`, the core SHALL send `session/set_config_option` setting the mode option to it if it is one of the option's values and not its current value, and SHALL then show the mode the answer reports. If `default_mode` is not one of the values, or the agent has no mode option, `*Messages*` SHALL get `<agent> has no mode <default_mode>` and nothing is sent. If the agent answers with an error, `*Messages*` SHALL get `<agent> could not set mode <default_mode>: <reason>`. In both cases the mode stays as the agent reported it.

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
