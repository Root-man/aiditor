# Tasks

## 1. Project file

- [x] 1.1 Add `env` to `Project` (and the ad-hoc constructor), checked in `Project::check`; verify with round-trip and bad-file tests in `project.rs` (non-string value, empty name, `=` in a name, NUL)
- [x] 1.2 Verify project commands leave `[env]` and its comments alone, extending the comment-preserving test in `project_commands.rs`

## 2. Processes

- [x] 2.1 `vterm` passes the current project's `env` as the spawn's extra variables; verify with a harness test that another project's terminal gets none
- [x] 2.2 `getenv` answers from the overrides first, before the capture state; verify with tests for a captured value overridden and an override while capturing
- [x] 2.3 Verify in the services loader test (`aiditor-services/src/projects.rs`) that a bad `env` file is rejected as `<file>: ...` naming the variable while the other files load; the core already shows loader errors as `project <file>: ...`

## 3. Docs and checks

- [x] 3.1 Update `docs/editor.md` (file format example, "Not yet" note, Process environment) and run `cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`
