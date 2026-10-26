## MODIFIED Requirements

### Requirement: The script names what the fake does
The fake agent SHALL read the JSON script file named by `AIDITOR_FAKE_AGENT`, relative to its working directory. The script is an object with an optional `session` list of steps, run once after the fake answers `session/new`, and an optional `prompts` list of step lists, where the n-th `session/prompt` runs the n-th list. Without `AIDITOR_FAKE_AGENT` the script is empty. A script that cannot be read or parsed SHALL make the fake write the reason to stderr and exit with code 2 before speaking ACP. On every start the fake SHALL report on stderr its pid, the file system and terminal capabilities the client advertised, the session's `cwd` and the variable `A`, and SHALL answer `session/new` with session id `fake-session` and one config option: a select with id `mode`, category `mode`, values `auto` and `plan`, current value `auto`.

#### Scenario: No script
- **WHEN** the fake is spawned without `AIDITOR_FAKE_AGENT` and opens a session
- **THEN** the session opens with id `fake-session` and a `mode` option with values `auto` and `plan`, current `auto`

#### Scenario: A bad script
- **WHEN** `AIDITOR_FAKE_AGENT` names a file that is not a valid script
- **THEN** the fake writes the reason to stderr and exits with code 2

### Requirement: Modes and cancel
The fake SHALL answer `session/set_config_option` for its `mode` option and a value it advertised with its config options, the mode set to that value, and then send a `config_option_update` with the same options; for any other option or value it SHALL answer with an invalid-params error. On `session/cancel` it SHALL stop the running turn at once, including a step that is sleeping or waiting for a permission answer, and answer that prompt with `cancelled`.

#### Scenario: Setting the mode
- **WHEN** the client sets the `mode` option to `plan`
- **THEN** the answer and a following `config_option_update` both have the `mode` option's current value `plan`

#### Scenario: An unknown value
- **WHEN** the client sets the `mode` option to `nope`
- **THEN** the request fails with an invalid-params error

#### Scenario: Cancelling a turn
- **WHEN** the client cancels a turn whose next step sleeps for a minute
- **THEN** the prompt is answered with `cancelled` within seconds
