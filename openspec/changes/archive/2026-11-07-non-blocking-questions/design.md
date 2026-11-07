# Design

## Decisions

- **Where the flag lives.** `parse` reads `_meta.codex.delivery` from the request and sets `Question::nonblocking`; codex-acp puts it on the request, not a field. Everything else about the question (fields, notes, answers) is unchanged, so steering and dropping late answers stay the adapter's job.
- **Status.** `Session::activity` counts only blocking asks; the mode line appends the non-blocking count. `session_state` in the sessions list follows the same split. Output already goes in at `Session::end`, below every block. A non-blocking question without a tool call ends the run before and after it (`end_run`), so later tool calls group below it instead of folding under a group header above it.
- **Point.** `show_question` takes the "prompt holds text" branch for a non-blocking question: echo, no move. The frame shows point in the prompt while the turn runs on.
- **Turn end.** codex-acp withdraws open questions when its turn ends, but the order against the `session/prompt` answer isn't specified. Closing them at the turn's end as `○ withdrawn` (answering `cancel`, which the adapter drops) gives the same line whichever arrives first, and closes them for an agent that never withdraws. A later withdrawal finds no ask and does nothing.
- **Hint.** Only the single-field, single-choice case takes frame 6f's hint; codex's async questions are single-choice, and a block of several keeps its own keys.

## Not done

- `#413` shapes the closed line (`Closed, the turn ended first`); this keeps `○ withdrawn`.
- The fake agent's `question` step still waits for its answer; core tests drive non-blocking questions through messages.
