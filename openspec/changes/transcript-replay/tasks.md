# Tasks

## 1. Replay mode

- [x] 1.1 Add the replay mode to `crates/aiditor-services/src/bin/aiditor-fake-agent.rs`: `AIDITOR_FAKE_AGENT_REPLAY`, method/id matching, id remapping, `{cwd}`, exit 1 on an unexpected message, 2 on a bad transcript, 0 on stdin closing; verify with `cargo build -p aiditor-services`

## 2. Tests

- [x] 2.1 In `crates/aiditor-services/tests/transcripts.rs`, replay every committed transcript through the agent service into the core and assert typed updates, answered requests and recorded stop reasons; verify with `cargo test -p aiditor-services --test transcripts`
- [x] 2.2 A test that a `session/cancel` the transcript doesn't expect fails the replay with the message shown

## 3. Docs and checks

- [x] 3.1 Describe replay in `docs/editor.md` Testing and in the transcripts README
- [x] 3.2 `cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D warnings` and `cargo test --workspace` pass
