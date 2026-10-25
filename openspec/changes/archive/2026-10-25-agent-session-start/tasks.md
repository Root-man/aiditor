# Tasks

## 1. Service

- [x] 1.1 Forward `session/update` notifications as `Msg::AgentUpdate` in `aiditor-services/src/agents.rs`, add the `update` behavior to `aiditor-test-agent`, and verify with a service test that the update reaches the core

## 2. Core sessions

- [x] 2.1 Add the `agents` module: `Session`, `Core::agents`, `agent-start` with registry completion and `default_agent` preselect, the environment layers and the wait on a running capture; verify with harness tests (preselect, env order, unknown agent, waiting, killed while waiting)
- [x] 2.2 Handle `AgentStarted`, `AgentStderr`, `AgentExited` and `AgentUpdate`: log buffer lines, `*Messages*` notes, ending a session with one kill; verify with harness tests (stderr logged, ready with modes, failed start, crash, one of two crashes, update kept)
- [x] 2.3 Add `sync_agents` to `Core::handle`, cover agent log buffers in the `kill-buffer` and quit questions; verify with harness tests (workspace kill sends a kill without a crash note, kill-buffer asks, quit asks with a live session)

## 3. Docs and checks

- [x] 3.1 Record the command, log buffer and session lifetime in `docs/editor.md` under ACP integration
- [x] 3.2 `cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D warnings` and `cargo test --workspace` pass
