## MODIFIED Requirements

### Requirement: Asking every agent for its sessions
`agent-sessions` (M-x) SHALL send one list request per enabled agent of the current workspace's project, in registry order, each with its own request number, the agent's command and arguments, the project's primary repo as working directory, and the environment an agent session of that project would start with. When the project's environment is being captured, the requests SHALL wait and SHALL be sent once that capture answers. With no enabled agent, it SHALL fail with `No agents enabled` and show nothing.

#### Scenario: One request per agent
- **WHEN** two agents are registered and enabled and `agent-sessions` runs in a project whose primary repo is `app`
- **THEN** one list request per agent is sent, each with `app` as working directory and its own request number

#### Scenario: The requests wait for the capture
- **WHEN** `agent-sessions` runs while the project's environment is being captured
- **THEN** no list request is sent until the capture answers, and then each is sent with the captured variables first

#### Scenario: A disabled agent is not asked
- **WHEN** the registry has `claude` and `codex`, the project enables only `claude`, and `agent-sessions` runs
- **THEN** one list request is sent, to `claude`, and no `Listing codex...` line shows

#### Scenario: No enabled agent
- **WHEN** the project sets `enabled_agents = []` and `agent-sessions` runs
- **THEN** the echo area shows `No agents enabled` and no sessions buffer is shown

### Requirement: Refreshing the list
`g` in the sessions buffer SHALL send a new list request per enabled agent as `agent-sessions` does, show `Listing <agent>...` for each until it answers, drop answers to the earlier requests, and reset the folds to their defaults.

#### Scenario: A late answer is dropped
- **WHEN** `g` is pressed before `claude` answered the first request, and then the first request's answer arrives
- **THEN** the buffer still shows `Listing claude...`, and the second request's answer shows its sessions
