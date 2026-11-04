## ADDED Requirements

### Requirement: Selecting the session's model
`agent-select-model` in a session buffer SHALL ask `Model: ` over the values of the session's model option, the first select config option of category `model`, by display name with groups flattened, the current value selected when the list opens and each value's description as its annotation. The options are the latest reported, as for `S-TAB`. Picking a name SHALL send `session/set_config_option` with the option's id and the first value with that name, and the mode line SHALL follow the answer.

#### Scenario: Picking a model
- **WHEN** the model option is `opus` with values `Opus` (`opus`) and `Sonnet` (`sonnet`), and `agent-select-model` is run and `Sonnet` picked
- **THEN** the list shows `Opus` selected and `Sonnet`, `model=sonnet` is sent, and after the answer the mode line shows `Sonnet`

#### Scenario: Grouped values
- **WHEN** the model option's values are in groups
- **THEN** the list shows every group's values in order

### Requirement: Model selection refusals
`agent-select-model` SHALL echo `<agent> has no model choice` in a session without a model option, `<agent> is not ready` before `session/new` is answered and `<agent> session has ended` after the agent exits; then no prompt opens and nothing is sent. A name no value has SHALL echo `<agent> has no model <name>` and send nothing. If the agent answers with an error, `*Messages*` SHALL get `<agent> could not set model <value>: <reason>` with the value id sent.

#### Scenario: No model option
- **WHEN** the session has no model option and `agent-select-model` is run
- **THEN** `claude has no model choice` is echoed, no prompt opens and nothing is sent

#### Scenario: A refused model
- **WHEN** the agent answers the set to `sonnet` with an error
- **THEN** `*Messages*` has `claude could not set model sonnet: ` followed by the reason
