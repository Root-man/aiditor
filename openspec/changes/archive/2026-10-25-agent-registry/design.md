# Design

## Context

Settings are bundled TOML with the user's file merged over it table by table (`settings::load`), deserialized into `Settings` with `deny_unknown_fields`, and rejected whole on any error. Project files are parsed and checked without the settings (`Project::parse`, also run in `aiditor-services`), then handed to the core in `Msg::ProjectsLoaded`, which `reconcile_projects` reports errors from. Each reload dispatches settings before projects.

## Goals / Non-Goals

**Goals:** a typed registry the spawn task can read from `Core::settings()`; the `default_agent` check.

**Non-Goals:** spawning agents, checking `default_mode` against the modes an agent advertises (only known after `session/new`), removing a bundled agent from the user file, a bundled Codex entry.

## Decisions

- **`agents: BTreeMap<String, Agent>` on `Settings`.** The existing recursive table merge gives "override one key or add an agent" for free. `BTreeMap` keeps names sorted for completion. Alternative, a separate `agents.toml`: rejected, the issue asks for settings.
- **Env checks shared with project files.** The project file's variable-name and NUL checks move into one function both call, so the two `env` tables accept the same names.
- **The `default_agent` check runs in the core, in `reconcile_projects`.** Only the core has both the settings in effect and the projects; the services' `Project::parse` does not see settings. Settings load before projects in every reload, so the check uses the new registry. It reports and keeps the project and its value: rejecting the whole file over a name would lose roots the user is working in, and dropping the value would erase it the next time a project command writes the file.
- **Bundled command is `claude-agent-acp` from `PATH`.** No `npx` wrapper: it adds network and startup latency on every session, and the user can override `command`/`args` in one line.
- **`project-set-default-agent` checks the name in the command.** Writes do not go through `reconcile_projects`, so without this a typo would land in the file silently until the next reload.

## Risks / Trade-offs

- [The bundled command is not on `PATH` on a fresh machine] → the spawn task's failure message names the command; the settings comment says how to point it elsewhere.
- [A user's `args` replaces the bundled list rather than extending it] → same as every other array setting; documented by example in the bundled file.
