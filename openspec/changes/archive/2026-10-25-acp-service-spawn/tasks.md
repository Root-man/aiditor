# Tasks

## 1. Core types

- [x] 1.1 Add `agent-client-protocol-schema` 1.12.0 (no default features) to the core; add `AgentId`, `AgentOp`, `Effect::Agent` and the four `Msg` variants; wrap `AgentRequest` in a boxed `Request` newtype with `PartialEq`, so `Msg` keeps its derive; verify `cargo check --workspace --all-targets`
- [x] 1.2 Reply to every `Msg::AgentRequest` with a "method not found" `AgentOp::Reply`; verify with a core test

## 2. Agents service

- [x] 2.1 Add `agent-client-protocol` 3.5.0 and `tokio-util` (compat) to services and `agents.rs` with `Agents::run` (spawn, reply, kill) and `kill_all`; verify it builds
- [x] 2.2 Add the `aiditor-test-agent` binary; verify it builds
- [x] 2.3 Add `tests/agents.rs` covering every scenario in `specs/agent-processes/spec.md`; verify they pass

## 3. App

- [x] 3.1 Run `Effect::Agent` in `editor.rs` (draining spawn receivers) and kill all agents on quit, plus in the benchmark teardown if it handles effects; verify `cargo build -p aiditor`
- [x] 3.2 Run `cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace` and the rest of CI; verify all pass
