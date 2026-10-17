# Tasks

## 1. Command tracking

- [x] 1.1 Add `this_command` and `last_command` to `Core` with public accessors, set them in `Ctx::key` (prefix keys unchanged, no-command sequences clear, `C-g` on a prefix records `keyboard-quit`); verify with harness tests for running a bound command from keys, last command across runs, prefix, typing, quit and nested calls

## 2. Key recording

- [x] 2.1 Add `recording: Option<Vec<Key>>` to `Core` with `start_recording` and `take_recording`, push every key at the top of `Ctx::key`; verify with harness tests for recorded order and not recording

## 3. Checks

- [x] 3.1 Run `cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D warnings` and `cargo test --workspace`, all passing
