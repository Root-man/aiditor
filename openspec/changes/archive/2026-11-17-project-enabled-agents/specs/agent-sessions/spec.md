## MODIFIED Requirements

### Requirement: Starting an agent session
`agent-start` SHALL prompt `Agent: `, completing over the current workspace project's enabled agents with its `default_agent` preselected when it names one of them, else the first name. When the project enables exactly one agent, `agent-start` SHALL start it without prompting. A name that is not registered SHALL be refused with `"<name>" is not a registered agent`, and a registered name the project does not enable with `"<name>" is not enabled in <project>`, both starting nothing. With no enabled agent the prompt SHALL complete over nothing. Otherwise it SHALL start the agent's command and args in the project's primary repo, with the environment variables in this order, later ones winning: the project's captured environment (none when nothing is captured or the capture fell back), the agent's `env` from the registry, the project file's `env`. The echo area SHALL show `Starting <agent>...`.

#### Scenario: The default agent is preselected
- **WHEN** the project's `default_agent` is `codex`, the registry has `claude` and `codex`, and `M-x agent-start RET RET` is typed
- **THEN** a spawn of `codex`'s command is requested in the primary repo

#### Scenario: The environment layers
- **WHEN** the captured environment has `PATH=/a` and `A=1`, the agent's `env` sets `A=2`, the project file sets `B=3`, and the agent is started
- **THEN** the spawn's variables are the captured ones, then `A=2`, then `B=3`, in that order

#### Scenario: An unknown agent
- **WHEN** `agent-start` is answered with a name that is not registered
- **THEN** the echo area shows `"<name>" is not a registered agent` and no spawn is requested

#### Scenario: Only enabled agents are offered
- **WHEN** the registry has `claude`, `codex` and `gemini`, project `p` sets `enabled_agents = ["claude", "codex"]`, and `M-x agent-start` is typed in a workspace of `p`
- **THEN** the prompt completes over `claude` and `codex` only

#### Scenario: A single enabled agent skips the prompt
- **WHEN** project `p` enables only `codex` and `M-x agent-start RET` is typed in a workspace of `p`
- **THEN** no `Agent: ` prompt opens, a spawn of `codex`'s command is requested, and the echo area shows `Starting codex...`

#### Scenario: A disabled agent is refused
- **WHEN** project `p` enables only `codex` and `agent-start` is answered `claude`
- **THEN** the echo area shows `"claude" is not enabled in p` and no spawn is requested
