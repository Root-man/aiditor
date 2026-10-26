# Tasks

## 1. Buffer

- [x] 1.1 Add the prompt boundary (`Buffer::prompt`, `Error::TextReadOnly`) and `insert_output` with undo shifting in `buffer.rs`; verify with unit tests (edits before the boundary refused, at it allowed, output keeps the prompt area's undo)

## 2. Service

- [x] 2.1 Add `AgentOp::Prompt`, `AgentOp::Cancel` and `Msg::AgentPrompted`; keep the connection in the agent handle and send prompts and cancels; verify with service tests on the fake agent (a turn streams then ends, a cancel ends the turn)

## 3. Core session buffer

- [x] 3.1 Create and show the session buffer from `agent-start`, make it the session's handle, keep stderr in the log buffer; update the existing harness tests
- [x] 3.2 Add `agent-session-mode` with `agent-send-input` (`RET`), `S-RET` newline and `agent-cancel` (`C-c C-c`); render text message chunks and turn ends; verify with harness tests (send, busy, not ready, ended, newline, streamed reply, refusal, error, cancel, read-only transcript, output keeps the draft)
- [x] 3.3 An end-to-end service test drives the core by keys against the fake agent: send a prompt, see the streamed reply and the turn end, cancel a turn, kill the session buffer and the workspace and check the agent is gone

## 4. Docs and checks

- [x] 4.1 Record the keys and behaviour in `docs/editor.md` under ACP integration
- [x] 4.2 `cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D warnings` and `cargo test --workspace` pass
