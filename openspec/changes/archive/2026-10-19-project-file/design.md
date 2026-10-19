# Design

## Context

See proposal.md, Why. Settings already load the same way (`settings::load`, `Msg::SettingsLoaded`, `Effect::LoadSettings`, `Core::report`). The core does no filesystem I/O; the app and `aiditor-services` do.

## Goals / Non-Goals

**Goals:** the format, parsing, validation, loading and error reporting; serialization for later writers.

**Non-Goals:** project commands (#58), workspaces using projects (#62), checking `default_agent` against an agent registry (none exists yet), per-project `env`, duplicate-name or overlapping-root checks (#57).

## Decisions

- **Schema.** `name: String`, `default_agent: Option<String>`, `primary: Root`, `roots: Vec<Root>` (default empty), `Root { path: PathBuf, role: String }` (role default empty). `deny_unknown_fields` everywhere, as settings do. Alternative: a `[[roots]]` list with a `primary = true` flag; rejected, since exactly one primary is then a runtime check instead of the type.
- **Parsing through the settings module.** `Project::parse(text)` calls `settings::load("", Some(text))` (no bundled defaults) and then checks name, absolute paths and one-line roles. Errors are `String`, like settings.
- **Existence check outside the core.** `aiditor_services::projects::read(dir)` lists `dir/*.toml` sorted by name, reads each, calls `Project::parse`, then checks every path `is_dir()`. It returns `Vec<Result<Project, String>>`, each error prefixed `<file name>: `. The core stays pure; alternative of passing an `is_dir` closure into the core was more plumbing for no gain.
- **Message.** `Msg::ProjectsLoaded(Vec<Result<Project, String>>)`: the core replaces `projects` with the `Ok`s and calls `report("project <err>")` per error, so they share the echo line like settings and keymap errors.
- **When.** At launch in `main.rs` next to settings, and in the app's `Effect::LoadSettings` arm. The test harness and the headless binary skip it (they have no services dependency / already skip `LoadSettings`); core tests send the message directly and services tests cover the reader.

## Risks / Trade-offs

- [A reload with an invalid file drops that project until fixed] → acceptable while nothing uses projects; revisit when workspaces bind to them (#62).
- [`default_agent` is unchecked] → `shortcut:` comment; validate once the agent registry lands.
