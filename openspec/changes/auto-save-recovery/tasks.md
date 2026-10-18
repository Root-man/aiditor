# Tasks

## 1. Services

- [x] 1.1 Switch `files::hash` to FNV-1a 64; verify with a test pinning a known hash
- [x] 1.2 Add `recovery::spawn`, the thread that writes and deletes recovery files; verify with a test that writes, rewrites and deletes one in a temp dir, leaving no temp file

## 2. Core

- [x] 2.1 Add `Msg::AutoSave`, `Effect::AutoSave`, `Effect::DeleteAutoSave`, the idle and 30 s timers and the delete sweep; verify with harness tests for every `auto-save` scenario
- [x] 2.2 Record the decisions in `docs/editor.md`

## 3. App

- [x] 3.1 Fill the recovery dir in `parse_args`, start the service, route the effects to it and join it before quitting

## 4. Checks

- [x] 4.1 `cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D warnings` and `cargo test --workspace` pass
