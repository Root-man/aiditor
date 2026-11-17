## ADDED Requirements

### Requirement: Enabled agents
The settings SHALL hold an optional `enabled_agents` list of agent names, and a project file MAY hold the same key, which replaces the settings' list for that project. A project's enabled agents SHALL be the registered agents its effective list names, in registry order; with no list in either, every registered agent SHALL be enabled. An empty list SHALL enable none.

#### Scenario: No list anywhere
- **WHEN** the registry has `claude` and `codex` and neither the settings nor the project file sets `enabled_agents`
- **THEN** the project's enabled agents are `claude` and `codex`

#### Scenario: Global list
- **WHEN** the registry has `claude` and `codex` and the settings set `enabled_agents = ["codex"]`
- **THEN** every project without its own list has `codex` as its only enabled agent

#### Scenario: Project list replaces the global one
- **WHEN** the settings set `enabled_agents = ["codex"]` and a project file sets `enabled_agents = ["claude"]`
- **THEN** that project's only enabled agent is `claude`, and other projects still have `codex`

#### Scenario: Registry order
- **WHEN** a project file sets `enabled_agents = ["codex", "claude"]`
- **THEN** its enabled agents are listed `claude`, then `codex`

### Requirement: A bad global enabled list rejects the settings
A settings `enabled_agents` that is not a list of strings, or that names an agent the merged registry does not hold, SHALL reject the settings file as any invalid setting does: the error names `enabled_agents` and the agent, and the previous settings stay in effect.

#### Scenario: Unregistered name
- **WHEN** the user file sets `enabled_agents = ["gemini"]` and no agent `gemini` is registered, and the settings are reloaded
- **THEN** an error naming `enabled_agents` and `gemini` is shown and the previous settings stay in effect
