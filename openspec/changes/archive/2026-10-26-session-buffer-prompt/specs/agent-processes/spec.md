# Spec Delta

## ADDED Requirements

### Requirement: Prompts and cancels reach the agent
The agent service SHALL send a prompt op's `session/prompt` request to the agent once its session is open and answer with the agent's response or its error message, after every `session/update` the agent sent before that response. A cancel op SHALL send the `session/cancel` notification. Both SHALL be ignored for an unknown agent or one whose session isn't open.

#### Scenario: A turn streams then ends
- **WHEN** a prompt is sent to a fake agent whose turn sends a message chunk then stops with `end_turn`
- **THEN** the core gets the chunk's update and then the prompt answer with `end_turn`

#### Scenario: A cancel ends the turn
- **WHEN** a prompt starts a turn that sleeps and a cancel is sent
- **THEN** the prompt answer has the stop reason `cancelled`
