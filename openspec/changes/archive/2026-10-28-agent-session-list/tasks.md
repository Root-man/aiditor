# Tasks

## 1. Core

- [x] 1.1 Add `AgentOp::List` and `Msg::AgentListed`; the core ignores `AgentListed`; update the harness op printer and other exhaustive matches

## 2. Service

- [x] 2.1 `Agents::run` handles `List`: spawn in its own group with stderr discarded, `initialize`, `session/list` through `nextCursor` up to 50 pages, answer, then terminate; failures answer an error
- [x] 2.2 Track running lists so `kill_all` kills them

## 3. Fake agent

- [x] 3.1 Script `sessions` pages: advertise `sessionCapabilities.list`, answer `session/list` by cursor filtered by `cwd`, invalid-params for an unknown cursor

## 4. Tests

- [x] 4.1 Service tests: every page listed in order without other cwds and the process exits; no capability; missing program; exiting agent (cargo test -p aiditor-services --test agents)
- [x] 4.2 Check against the real claude-agent-acp and codex-acp on macOS

## 5. Docs

- [x] 5.1 Record the built behavior in `docs/editor.md` (Session manager, Fake agent script)
