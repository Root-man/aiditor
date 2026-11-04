## MODIFIED Requirements

### Requirement: Modes and cancel
The fake SHALL advertise a `mode` select option and a `model` select option of category `model` with values `default` and `fast`, current `default`. It SHALL answer `session/set_config_option` for either option and a value it advertised for it with its config options, that option set to that value and the other kept, and then send a `config_option_update` with the same options; for any other option or value it SHALL answer with an invalid-params error. On `session/cancel` it SHALL stop the running turn at once, including a step that is sleeping or waiting for a permission answer, and answer that prompt with `cancelled`.

#### Scenario: Setting the mode
- **WHEN** the client sets the `mode` option to `plan`
- **THEN** the answer and a following `config_option_update` both have the `mode` option's current value `plan`

#### Scenario: Setting the model
- **WHEN** the client sets the `model` option to `fast` and then the `mode` option to `plan`
- **THEN** both answers have the `model` option's current value `fast`

#### Scenario: An unknown value
- **WHEN** the client sets the `mode` option to `nope`
- **THEN** the request fails with an invalid-params error

#### Scenario: Cancelling a turn
- **WHEN** the client cancels a turn whose next step sleeps for a minute
- **THEN** the prompt is answered with `cancelled` within seconds
