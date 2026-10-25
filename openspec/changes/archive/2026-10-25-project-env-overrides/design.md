# Design

## Context

`Project` is parsed by serde with `deny_unknown_fields`, then `Project::check` validates what serde can't. Terminals already accept extra variables on top of the editor's environment (`TerminalOp::Spawn::env`, empty today). The captured environment lives per project in `Core::environments`; no process uses it yet.

## Goals / Non-Goals

**Goals:** the project file's `env` reaches terminals and `getenv`; bad entries reject the file.

**Non-Goals:** unsetting variables, `${VAR}` expansion, the agent registry's `env`, passing the captured environment to terminals (their login shell loads the profile itself).

## Decisions

- **`env: BTreeMap<String, String>`**, `#[serde(default, skip_serializing_if = "BTreeMap::is_empty")]`. A non-string value is a TOML type error, reported by the existing path. Sorted order makes spawns deterministic; the order of a map doesn't matter to a process.
- **Checks in `Project::check`**: an empty name, a name with `=` or NUL, a value with NUL, each as `env <name>: <reason>`. The OS refuses these at spawn time, so the file is the place to say so.
- **`sync` leaves `env` alone**, as it does `ignore`: project commands only rewrite what they edit, so the key and its comments stay.
- **Terminals get the overrides as extra variables.** The terminal service already layers extras over the editor's environment and sets its own terminal variables after, so the terminal variables still win.
- **`getenv` looks in the overrides first**, then the captured or inherited environment, so it shows what a process will see. A capture still running still reports that, since the rest of the environment is unknown; an overridden variable is answered at once.

## Risks / Trade-offs

- [A login shell's profile can reassign a variable after the terminal starts] → expected for shells; the overrides are the starting environment, and agents and formatters, which run no profile, see them as set.
- [A hand edit needs `C-x C-r`] → same as every other project key.
