# Spec Delta

## ADDED Requirements

### Requirement: Session updates stream to the core
Each `session/update` notification the agent sends SHALL reach the core as one message tagged with the agent id, holding the notification as ACP parses it, in the order the agent sent them.

#### Scenario: An update reaches the core
- **WHEN** the test agent sends one session update after its session opened
- **THEN** the core gets an update message for that agent id with the update's session id and content
