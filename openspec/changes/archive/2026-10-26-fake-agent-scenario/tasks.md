# Tasks

## 1. Scenario

- [x] 1.1 Add `crates/aiditor-services/tests/scenario.rs`: a temporary project with an open file, settings registering the fake agent, a key script running `agent-start` and a prompt; assert the reply in the session buffer, the open buffer reverting to the fake's write, and no agent process after `C-x t 0`; verify with `cargo test -p aiditor-services --test scenario` passing
- [x] 1.2 Name the scenario in `docs/editor.md`'s Testing section; verify by reading the section

## 2. Checks

- [x] 2.1 `cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D warnings` and `cargo test --workspace` pass
