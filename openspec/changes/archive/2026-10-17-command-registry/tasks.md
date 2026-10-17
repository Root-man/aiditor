# Tasks

## 1. Registry

- [x] 1.1 Add `crates/aiditor-core/src/command.rs` with `Command`, `ArgSpec`, `Args` and the handler type, and export it from `lib.rs`; verify `cargo build -p aiditor-core`.
- [x] 1.2 Give `Core` the registry, `Core::register` and `Ctx::call` (unknown name, region fill and no-mark error, missing prompt strings, handler errors to the message line); verify with harness tests for each scenario in `specs/commands/spec.md`.

## 2. Built-in commands

- [x] 2.1 Register `undo`, `undo-redo`, `save-buffer` and `reload-settings` and route their keys through `Ctx::call`; verify the existing harness tests still pass and a test calls `undo` by name.

## 3. Checks

- [x] 3.1 Run `cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D warnings` and `cargo test --workspace`, and the rest of `.github/workflows/ci.yml`; all pass.

## Workflow follow-up

- Archive the change after verify reports no CRITICAL.
