# Proposal

## Why

Every agent in the registry is offered in every project: `agent-start` completes over all of them and `agent-sessions` spawns each one to list its sessions. A project that only uses one agent still pays for the others in the prompt and in a throwaway process per listing, and a bundled agent such as `claude` can't be turned off at all, since user tables only merge over it.

## What Changes

- A new optional `enabled_agents` list of agent names in `settings.toml`: the agents offered everywhere. Unset means every registered agent, as today.
- The same optional key in a project file replaces the global list for that project, as `max_watched_files` does.
- `agent-start` completes over, preselects from and accepts only the current project's enabled agents.
- `agent-start` starts the project's only enabled agent without asking.
- `agent-sessions` and `g` in `*Sessions*` ask only the enabled agents.
- `project-set-default-agent` completes over and accepts only the project's enabled agents.
- A settings `enabled_agents` naming an unregistered agent rejects the settings; a project's unknown name, or a `default_agent` outside its enabled agents, is reported when projects load and the project still loads.

Not included: a command to edit the list (hand edit and `C-x C-r`), per-project agent definitions (command, env), and stopping live sessions of an agent that becomes disabled.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `agent-registry`: adds the global `enabled_agents` list, the project override and the resulting set of enabled agents.
- `agent-sessions`: `agent-start` uses the project's enabled agents instead of every registered one.
- `sessions-list`: listing and refreshing ask the project's enabled agents instead of every registered one.
- `projects`: the project file gains `enabled_agents`; `project-set-default-agent` and the load-time agent report use the enabled agents.

## Impact

- `crates/aiditor-core/src/settings.rs`, `settings.toml` (commented key): new setting and its check.
- `crates/aiditor-core/src/project.rs`: new project field, kept by `sync` as `ignore` and `env` are.
- `crates/aiditor-core/src/agents.rs` (`agent_names`, `default_agent_index`, `start_session`), `sessions.rs` (`agent_sessions`), `builtins.rs` (`projects_then_agents`), `project_commands.rs` (`reconcile_projects`, `set_default_agent`).
- `docs/editor.md`: the agent registry and project file sections.
- No new dependencies; existing files without the key behave as before.
