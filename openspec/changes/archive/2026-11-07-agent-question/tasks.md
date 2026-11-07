# Tasks

## 1. Agent service

- [x] 1.1 Advertise `elicitation.form` in `initialize`; verify the fake reports `elicitation.form=true elicitation.url=false` in the `agents.rs` service test that checks the capabilities
- [x] 1.2 Watch each pending request's cancellation: answer it with the request-cancelled error, drop it from the pending map and send `Msg::AgentWithdrawn { id, request }`; verify with a service test where the fake withdraws a question

## 2. Fake agent

- [x] 2.1 Add the `question` step with optional `withdrawAfter`, and report the elicitation capabilities; verify with the `agents.rs` service test where a withdrawn question writes `question error: Request cancelled`

## 3. Core

- [x] 3.1 Give `Ask` a body (permission options or a parsed question) and parse single-choice questions (design 2); decline other forms with the echo line; verify with core tests for the Claude and Codex shapes and the undrawable cases
- [x] 3.2 Render the question block and its answered lines (design 3); status `● awaiting answer`; the block stands in for its call's group row; verify with core tests on the block text and status
- [x] 3.3 Keys: `agent-question-key` on `1`..`9` and `o`, `agent-answer-own`, `agent-goto-question` on `C-c C-q`; `C-c C-k` skips questions, `C-c C-y` passes them by; verify with core tests for each answer's content and the self-insert fallback
- [x] 3.4 Point on arrival and the waiting echo (design 5); verify with core tests for an empty and a non-empty prompt
- [x] 3.5 Cancel on `C-c C-c`, turn end and buffer kill (permission requests too); close withdrawn blocks; verify with core tests

## 4. Whole app and docs

- [x] 4.1 A `scenario.rs` test answering questions from the fake agent with a digit, an own answer, `C-c C-k`, and a withdrawal
- [x] 4.2 Record the question rule and decisions in `docs/editor.md` (ACP table, display model, fake agent steps); verify by rereading against the specs
- [x] 4.3 `cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace` pass
