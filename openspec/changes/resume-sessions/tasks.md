# Tasks

## 1. Agent service

- [x] 1.1 Add `session: Option<SessionId>` to `AgentOp::Spawn` and pass `None` everywhere it is built (core, bench, service tests, transcript test); verify `cargo build --workspace --all-targets`
- [x] 1.2 Fake agent: a script `history` advertises `loadSession` and answers `session/load` (stderr `load=<id>`, `cwd=<cwd>`, the history for that id, then the config option); verify through 1.3's tests
- [x] 1.3 In `connect`, send `session/load` for a spawn with a session, failing with `agent can't load sessions` without the capability, and answer `AgentStarted` with the requested id and the load answer's options; verify new service tests for a load with history and for an agent that can't load

## 2. Core

- [x] 2.1 Render `user_message_chunk` text as a `> ` block; verify a test with agent, user and agent chunks
- [x] 2.2 Share `agent_start`'s body with loading, add `Session::load`, match it in `open_session`, and skip `default_mode` for a loaded session; verify a test that a loaded session with `default_mode` sends nothing
- [x] 2.3 `RET` on a row not open loads it in the current workspace with its listed title; verify tests replacing `Loading sessions isn't supported yet`: the spawn carries the session and primary root, the buffer name, `[loading]` then `[idle]`, the list staying shown until the load answers, a second `RET` spawns nothing

## 3. Docs and checks

- [x] 3.1 Record the load in `docs/editor.md`'s session manager paragraph and the fake agent's script keys under Testing
- [x] 3.2 `cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace` and the rest of CI pass
