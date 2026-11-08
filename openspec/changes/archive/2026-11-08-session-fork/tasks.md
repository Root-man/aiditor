# Tasks

## 1. Service

- [x] 1.1 Enable `unstable_session_fork` on `agent-client-protocol`, which enables it on the schema crate
- [x] 1.2 Replace `AgentOp::Spawn`'s `session: Option<SessionId>` with `Open { New, Load, Fork }` and update every constructor
- [x] 1.3 In `connect`, fork then load for `Open::Fork`, failing with `agent can't fork sessions` without the capability
- [x] 1.4 Fake agent: `"fork": true` advertises fork, reports `fork=`, `at=`, `dirs=`, answers `fake-fork` and cuts the next load's history
- [x] 1.5 Service tests: whole fork, fork at a message (meta and cut replay), agent that can't fork

## 2. Core

- [x] 2.1 `Session::load` holds `Option<SessionId>`, none for a fork; `start_session` takes the open kind and echoes `Forking <agent> session...`
- [x] 2.2 `agent-fork-session`: fork point from point, the refusals, registered in builtins
- [x] 2.3 A failed fork echoes `<agent> fork failed: <error>` and kills its buffers
- [x] 2.4 Core tests for each spec scenario, including the source still sending after the fork

## 3. Docs

- [x] 3.1 Record the decisions in `docs/editor.md` (ACP section) and the fake agent's `fork` key under Testing
