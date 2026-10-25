# Proposal

## Why

A project sometimes needs variables its shell profile does not set (`MIX_ENV`, `RUST_LOG`, an API endpoint), and they belong to the project, not to the user's shell. The order was decided in #194 and #267 (docs/editor.md, Process environment): the agent registry's `env` goes on top of the captured environment and the project file's `env` on top of that. This change reads the project file's `env` (#269).

## What Changes

- The project file takes an optional `[env]` table of string variables.
- A bad `env` (a value that is not a string, an empty name, a name holding `=` or NUL, a value holding NUL) rejects the file like any other project file error; the other files still load.
- Processes started for the project get those variables on top of its environment: terminals started with `vterm` get them as extra variables, on top of the environment they already start with.
- `getenv` shows the project's environment with the overrides on top of the captured (or inherited) one.
- Project commands (`project-save`, root and role edits) leave `env` as it is in the file; a hand edit takes effect for processes started after `C-x C-r`.

## Capabilities

### New Capabilities

### Modified Capabilities
- `projects`: the project file format gains `env`, and its checks join the rejected-file rules.
- `process-environment`: the project's environment includes the project file's overrides, which reach terminals and `getenv`.

## Impact

- `aiditor-core`: `Project` gains `env` and its checks; `vterm` and `getenv` apply it.
- `docs/editor.md`: the project file format and Process environment note what was built.
- The agent registry's `env` is merged by the registry epic, in the order above.
