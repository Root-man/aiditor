## ADDED Requirements

### Requirement: Loading a session
A spawn MAY name a session id to load. Such a spawn SHALL start the agent and send `initialize` as any spawn does, then, when the agent advertises `loadSession`, send `session/load` with that session id, `cwd` set to the working directory and no MCP servers, instead of `session/new`. The `session/update`s the agent sends while loading, its replayed history, SHALL reach the core as session updates before the started message. The started message SHALL hold the requested session id and the config options and modes of the `session/load` answer. An agent that doesn't advertise `loadSession` SHALL fail to start with `agent can't load sessions`.

#### Scenario: A session is loaded
- **WHEN** the test agent with a two-update history is spawned to load session `old`
- **THEN** the agent saw `session/load` for `old` with the spawn's `cwd`, and the core gets both updates, then a started message with session id `old` and the agent's config options

#### Scenario: An agent that can't load
- **WHEN** the test agent without a history is spawned to load a session
- **THEN** the core gets a failure started message with `agent can't load sessions`
