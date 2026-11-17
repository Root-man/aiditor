# Design

## Context

The registry is `Settings::agents`, a `BTreeMap` keyed by name; every place that offers or asks agents iterates it directly: `Core::agent_names` and `default_agent_index` (agent-start's prompt), `start_session`, `agent_sessions` and `g`, and `projects_then_agents` (project-set-default-agent's prompt). `max_watched_files` already sets the pattern for a global setting a project file overrides: `Option<usize>` on `Project`, read as `project.or(settings)`.

## Goals / Non-Goals

**Goals:** one place that answers "which agents does this project enable", used by every caller above.

**Non-Goals:** editing the list with a command; per-project agent definitions; ending live sessions of an agent that becomes disabled (they keep running, their buffers keep working, and `*Sessions*` just stops listing that agent).

## Decisions

- **One helper on `Core`**: `enabled_agents(&Project) -> Vec<&str>`: registry keys, in registry order, kept when `project.enabled_agents.or(settings.enabled_agents)` is `None` or contains them. `agent_names` becomes this for the current project, and the other call sites switch from `settings.agents` to it. Alternative: filter `settings.agents` into a per-project map on load; rejected, since the settings and projects reload separately and the map would go stale.
- **A single enabled agent skips the prompt through a new `ArgSpec::single` flag**: when set and a declared prompt's completion returns exactly one candidate, the command loop fills that argument without opening the minibuffer. Only `agent-start` sets it. Alternatives: a rule for every prompt, rejected because it would also change `kill-buffer` and the project prompts; having the handler ask itself, rejected because it moves the prompt, completion and preselection out of the declared `ArgSpec`, which is more code.
- **Replace, not merge**: a project's list replaces the global one, as `max_watched_files` does and as the user asked ("overwrite"). A merge (add/remove) syntax is more to learn for no stated need.
- **Global list checked strictly, project list leniently**: the global list sits in the same file as the registry, so a wrong name is a typo the settings error catches at once. A project's list is checked against settings that can change later, so it is reported and kept like `default_agent` already is (`reconcile_projects`).
- **Spawning by name checks the enabled set** in `start_session` only for `Open::New`. Loading a session from `*Sessions*` only offers rows of enabled agents, so it needs no check of its own.
- **`sync` leaves the key alone**: project commands don't set `enabled_agents`, so `project::sync` needs no change, as for `ignore` and `env`; serde's `skip_serializing_if` keeps saved ad-hoc projects free of it.

## Risks / Trade-offs

- [A settings reload that removes an agent named by the global list now rejects the whole settings file] → the error names `enabled_agents` and the agent, so the fix is one edit; same behaviour as other bad settings.
- [`enabled_agents = []` leaves `agent-start` with nothing to complete] → `agent-sessions` says `No agents enabled`; `agent-start` refuses any typed name with the not-enabled message.
