# Tasks

## 1. Services

- [x] 1.1 Advertise `clientCapabilities._meta.terminal_output: true` in `initialize`; the fake agent reports `terminal_output=<bool>`; `a_session_opens_in_cwd_without_fs_or_terminal` asserts it (cargo test -p aiditor-services --test agents)

## 2. Core

- [x] 2.1 Open a terminal on a tool call from `_meta.terminal_info` or a terminal content item; append output under both keys and record `terminal_exit` from `tool_call` and `tool_call_update` `_meta`, dropping unknown terminals
- [x] 2.2 Render the terminal's output (ANSI and CR stripped) as `Output:` in place of `Terminal <id>`, and the exit in the header
- [x] 2.3 Defer output-only updates to the frame via `render_streams`
- [x] 2.4 Core unit tests: streamed chunks under both keys, once per frame, exit code, signal, clean exit, ANSI stripping, unannounced terminal (cargo test -p aiditor-core agents)

## 3. Integration and docs

- [x] 3.1 Fake-agent test in `tests/agents.rs`: a terminal tool call, chunks under both keys and a failing exit end up in the session buffer
- [x] 3.2 Committed transcripts still replay (`cargo test -p aiditor-services --test transcripts`)
- [x] 3.3 Record the built behavior under Agent command output and the fake agent's report in `docs/editor.md`
