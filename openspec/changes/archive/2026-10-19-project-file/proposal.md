# Proposal

## Why

Workspaces (#62) and project commands (#58) need projects to exist: a file format the editor reads, validates and later writes. This is issue #56.

## What Changes

- A project file format: TOML in `<config dir>/projects/*.toml` with `name`, optional `default_agent`, a `[primary]` root and `[[roots]]` additional roots, each root a `path` and a one-line `role`.
- The editor loads every project file at launch and again on `C-x C-r`, keeps the valid ones and reports each invalid file in the echo area (and `*Messages*`) with the file and the reason.
- Validation: valid TOML, no unknown keys, non-empty name, absolute paths that exist as directories, roles on one line.
- Projects serialize back to the same format, so later commands can write them.

## Capabilities

### New Capabilities
- `projects`: the project file format, loading, validation and error reporting.

### Modified Capabilities
None.

## Impact

- `aiditor-core`: new `project` module, a `Msg::ProjectsLoaded` message, a `Core::projects()` accessor.
- `aiditor-services`: a reader that lists and parses project files and checks their paths exist.
- `aiditor` app: sends the projects at launch and on the `LoadSettings` effect.
- `docs/editor.md`: the Projects section records the format.
