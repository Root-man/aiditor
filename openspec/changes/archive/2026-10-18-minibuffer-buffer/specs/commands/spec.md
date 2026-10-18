# Spec Delta

## MODIFIED Requirements

### Requirement: Prompted strings must be supplied
A command whose argument spec lists prompts SHALL receive one string per prompt. A call with fewer strings than prompts SHALL NOT run the handler; it SHALL read the next string in the minibuffer and call the command again with it.

#### Scenario: Missing string
- **WHEN** a command with one prompt is called with no strings
- **THEN** the handler does not run and the minibuffer opens with that prompt
