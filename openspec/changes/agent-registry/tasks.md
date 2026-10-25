# Tasks

## 1. Registry

- [x] 1.1 Add `Agent` (`command`, `args`, `env`, `default_mode`) and `Settings::agents` in `settings.rs`, with the bundled `[agents.claude]` in `settings.toml`; verify with tests that the bundled entry parses, a user key overrides it and a user entry adds an agent
- [x] 1.2 Reject an empty command and bad `env` variables, sharing the project file's env check; verify with cases in `bad_files_are_rejected`

## 2. Projects

- [x] 2.1 Report a project whose `default_agent` is not registered in `reconcile_projects`, keeping the project; verify with a harness test that the error reads `project <file>: ...` and the project loads, and that adding the agent and reloading clears it
- [x] 2.2 Complete `project-set-default-agent` over the registry and refuse an unregistered name; verify with a harness test

## 3. Docs

- [x] 3.1 Record the decision in `docs/editor.md` (agent registry bullet, drop the `default_agent` "not yet"); verify by reading the section
- [x] 3.2 Run `cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D warnings` and `cargo test --workspace`, all passing
