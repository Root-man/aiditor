# Tasks

## 1. Clock and tick plumbing

- [x] 1.1 Add `Msg::Clock`, `Core::clock` and the last `now`; `start_clock` and startup send the text. Verify with a GPUI test that the core's clock follows the frontend's
- [x] 1.2 Move the shown-buffers set out of `Core::frame`; arm and cancel one 1 s repeating timer in `handle`. Verify with a core test: hidden turn, no `next_deadline`; shown, a deadline; turn ended, none

## 2. Working line

- [x] 2.1 Add `Session::timer` and the transcript-end helper; route every output site through it; add the line on send. Verify output goes in above it
- [x] 2.2 Refresh shown sessions' lines from `render_streams` with a minimal edit. Verify ticking, a hidden session staying stale and current when shown, and point and prompt text staying put
- [x] 2.3 Settle to `Worked` with the clock on `end_turn`, other stop reasons, errors, cancel and exit, ending the stream first. Verify each with a test

## 3. Times

- [x] 3.1 Stamp messages and write the time on the echo and at `end_stream`. Verify live messages, the stamp being arrival time, and the message bar's lines
- [x] 3.2 Write the `Thought for` header at `end_stream`. Verify the duration and a hidden stream
- [x] 3.3 Add tool duration and waiting to the header; `Ask` keeps its start; refresh waiting per tick. Verify duration, a wait excluded, the counter ticking and ending, folds staying, and an ask with no shown call
- [x] 3.4 Skip all of it during replay. Verify with a replayed message, thought and tool call, and a live update after the load answer

## 4. Finish

- [x] 4.1 Update session text assertions in core and `crates/aiditor-services/tests/agents.rs`; verify `cargo test --workspace` passes
- [x] 4.2 Record the behaviour in `docs/editor.md`, replacing the Working timer paragraph's plan with what was built; verify by reading the section
