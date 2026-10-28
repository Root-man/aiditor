# Tasks

## 1. Protocol plumbing

- [x] 1.1 Add `roots` to `AgentOp::Spawn` and `Msg::AgentUnscoped`; fill roots in `start_session` and update every constructor/matcher; `cargo build --workspace --all-targets` passes
- [x] 1.2 Service: send `additionalDirectories` on `session/new`/`session/load` when advertised, else send `AgentUnscoped` before `AgentStarted` when roots were non-empty; verified by service tests in 3.2

## 2. Core

- [x] 2.1 Handle `AgentUnscoped` by logging `<agent> can't take additional directories`; core test checks `*Messages*`
- [x] 2.2 Build the preamble at start for new sessions, send it as the first block of the first prompt only; core tests for first/later/loaded/nothing-to-list prompts and buffer text

## 3. Fake agent and service tests

- [x] 3.1 Fake agent `directories` key and `dirs=` stderr lines; covered by 3.2
- [x] 3.2 Service tests: roots reach `session/new` and `session/load` with the capability, none plus `AgentUnscoped` without it

## 4. Docs

- [x] 4.1 Record the decisions in docs/editor.md (ACP section and fake agent section)
