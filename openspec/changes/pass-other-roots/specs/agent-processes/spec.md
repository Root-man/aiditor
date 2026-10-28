# Spec Delta

## ADDED Requirements

### Requirement: Additional directories reach the agent
A spawn SHALL carry the project's other roots. When the agent advertises `sessionCapabilities.additionalDirectories`, the service SHALL send them as `additionalDirectories` on `session/new`, or on `session/load` when loading. Otherwise it SHALL send none and, when there were roots to send, report to the core that the agent can't take additional directories before reporting the start.

#### Scenario: The agent takes them on session/new
- **WHEN** a spawn with roots `b` and `c` starts an agent advertising the capability
- **THEN** its `session/new` carries `additionalDirectories` `b` and `c`

#### Scenario: The agent takes them on session/load
- **WHEN** a spawn loading a session with root `b` starts an agent advertising the capability
- **THEN** its `session/load` carries `additionalDirectories` `b`

#### Scenario: The agent can't take them
- **WHEN** a spawn with root `b` starts an agent not advertising the capability
- **THEN** `session/new` carries no additional directories, and the core hears that the agent can't take them before the start

#### Scenario: No roots to send
- **WHEN** a spawn without other roots starts an agent not advertising the capability
- **THEN** the core hears only the start
