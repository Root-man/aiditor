# Tasks

## 1. Service

- [x] 1.1 Add `Open::Rollback(SessionId, MessageId)`; in `connect`, fork and load it as a fork at the message, then send `session/delete` of the source when the agent advertises delete
- [x] 1.2 Fake agent: `"delete": true` advertises delete and reports `delete=<id>`
- [x] 1.3 Service tests: rollback with delete, rollback without delete

## 2. Core

- [x] 2.1 Share the message-at-point lookup between fork and rollback
- [x] 2.2 `agent-rollback-session`: refusals, the `no`/`yes` question, spawn `Open::Rollback`, registered in builtins
- [x] 2.3 `Session::rollback`; on the load's answer swap windows, kill the source's buffers, rename the fork's; failure echoes `rollback failed`
- [x] 2.4 Core tests for each spec scenario

## 3. Docs

- [x] 3.1 Record the decisions in `docs/editor.md` (ACP section) and the fake agent's `delete` key under Testing
