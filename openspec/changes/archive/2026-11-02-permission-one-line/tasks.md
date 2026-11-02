# Tasks

## 1. Core

- [x] 1.1 Replace `Chose ...` and `Cancelled permission for ...` with the kind's line and `✗ cancelled`, drawn in `session-thought`; verified by the permission tests in `agents.rs`
- [x] 1.2 Answer pending requests `cancelled` in `agent_prompted`; verified by a new test for an end-of-turn stop reason and for an error

## 2. Tests and docs

- [x] 2.1 Update `scenario.rs` and `transcripts.rs`, and cover each kind and the line's face; `cargo test --workspace` passes
- [x] 2.2 Update the permission prompts text in `docs/editor.md`
