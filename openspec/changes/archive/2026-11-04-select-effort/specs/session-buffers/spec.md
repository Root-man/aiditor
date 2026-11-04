## ADDED Requirements

### Requirement: Selecting the session's effort
`agent-select-effort` in a session buffer SHALL ask `Effort: ` over the values of the session's effort option, the first select config option of category `thought_level`, by display name with groups flattened, the current value selected when the list opens and each value's description as its annotation. The values SHALL be the latest the agent reported, so after a model change they are the new model's. Picking a name SHALL send `session/set_config_option` with the option's id and the first value with that name, and the mode line SHALL follow the answer. It SHALL echo `<agent> has no effort choice` in a session without an effort option, `<agent> is not ready` before `session/new` is answered and `<agent> session has ended` after the agent exits, and then open no prompt and send nothing. A name no value has SHALL echo `<agent> has no effort <name>`. If the agent answers with an error, `*Messages*` SHALL get `<agent> could not set effort <value>: <reason>`.

#### Scenario: Picking an effort
- **WHEN** the effort option is `high` with values `Low` (`low`) and `High` (`high`), and `agent-select-effort` is run and `Low` picked
- **THEN** the list shows `Low` and `High` selected, `effort=low` is sent, and after the answer the mode line shows `Low`

#### Scenario: Efforts follow the model
- **WHEN** a model change reports an effort option with the values `Low` and `Max`
- **THEN** `agent-select-effort` lists `Low` and `Max`

#### Scenario: No effort option
- **WHEN** the session has no effort option and `agent-select-effort` is run
- **THEN** `claude has no effort choice` is echoed, no prompt opens and nothing is sent
