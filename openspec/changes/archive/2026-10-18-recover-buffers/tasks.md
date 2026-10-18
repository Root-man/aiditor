# Tasks

## 1. Services

- [x] 1.1 Add `recovery::list` and `Effect::DeleteRecovery` handling (skipping own-pid names); verify with a test that lists files written by the service, skips a `.tmp`, sorts by time and deletes by path

## 2. Core

- [x] 2.1 Add `Recovered`, `Msg::Recover`, `Effect::DeleteRecovery` and the `recover-buffers` command; verify with harness tests for every scenario of the `auto-save` delta
- [x] 2.2 Record the decisions in `docs/editor.md` Crash safety

## 3. App

- [x] 3.1 List the recovery dir in `main` and send `Msg::Recover`; route `Effect::DeleteRecovery` to the service; verify by a crash and relaunch on Linux (manual)

## 4. Checks

- [x] 4.1 `cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D warnings` and `cargo test --workspace` pass
