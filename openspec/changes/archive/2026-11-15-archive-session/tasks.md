## 1. Services

- [x] 1.1 `AgentOp::List` gains `delete: Option<(SessionId, Option<AgentId>)>`; update every construction and match
- [x] 1.2 The list stops the named agent as `Kill` does, and after `initialize` waits for its group, checks `sessionCapabilities.delete`, sends `session/delete`, then lists; share the group wait with `AgentOp::Delete`
- [x] 1.3 The fake agent drops deleted sessions from later `session/list` answers

## 2. Core

- [x] 2.1 `agent-sessions-archive` on `a`: `codex` rows send the list request with the delete (and the open session's agent), bump the listing's request, `forget_listed`; other agents `Not supported yet`; hint `a archive`

## 3. Tests

- [x] 3.1 Services: deleted session doesn't list; can't delete fails; the stopped agent exits
- [x] 3.2 Core: archiving codex (request, row gone, answer lands), claude fails, open session named and buffer kept, heading fails, hints

## 4. Docs

- [x] 4.1 `docs/editor.md`: Session manager records what #486 built
