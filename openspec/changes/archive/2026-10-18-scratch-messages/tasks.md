# Tasks

## 1. Core

- [x] 1.1 Add `Buffer::append` (end insert, no undo, past read-only) and verify a buffer unit test
- [x] 1.2 Add `Core::echo`/`log`, route every echo-area message through it, make `*Messages*` on demand read-only, and verify harness tests for the messages-buffer scenarios
- [x] 1.3 Add `Msg::Log` and verify a harness test that it appends a line

## 2. Frontend

- [x] 2.1 Make the services channel in `main`, add the ANSI-free `fmt` layer writing `Msg::Log` with `try_send`, and verify a test that a `tracing::info!` reaches the channel as a `Msg::Log`
- [x] 2.2 Note in `docs/editor.md` what was built, and verify fmt, clippy and tests pass

## Workflow follow-up

- Archive the change after the PR is reviewed.
