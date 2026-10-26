# Tasks

## 1. Recorder

- [x] 1.1 Add `crates/aiditor-services/src/bin/aiditor-acp-record.rs`: pass-through, JSON-line log with direction, exit code, usage error; verify with `cargo build -p aiditor-services`
- [x] 1.2 Test in `crates/aiditor-services/tests/transcripts.rs`: the agent service spawns the recorder in front of the fake agent, a session opens and a prompt is answered, and OUT holds both directions; bad usage exits 2; verify with `cargo test -p aiditor-services --test transcripts`

## 2. Transcripts

- [x] 2.1 Ignored `record_claude` and `record_codex` drivers in `tests/transcripts.rs` that run the scripted session through the recorder and write scrubbed transcripts; verify by running them with `-- --ignored`
- [x] 2.2 Record, review by hand for secrets, and commit `claude.jsonl` and `codex.jsonl` under `tests/transcripts/`, with a README on recording and scrubbing; a test checks every committed transcript parses and holds no home path
- [x] 2.3 Describe the recorder and transcripts in `docs/editor.md` Testing; verify by reading the section

## 3. Checks

- [x] 3.1 `cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D warnings` and `cargo test --workspace` pass
