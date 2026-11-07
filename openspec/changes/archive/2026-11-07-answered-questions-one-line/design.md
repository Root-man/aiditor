## Context

`close_ask` replaces a pending ask's block with a dim line through `settle` and frees `Ask::start`; `Row::Ask { start, len }` keeps the line's span as a group row, and `end_run` frees the markers of rows no group kept, so a lone line is untracked after its run. A question's tool call keeps its own `Tool` block; `Session::hidden` hides it only while folded in a group.

## Goals / Non-Goals

**Goals:** the line's shape and faces from frame 6f, `TAB` on it, the call's row gone.

**Non-Goals:** permission requests' lines; changing the group line's wording for a call it stands for; reopening an answered question.

## Decisions

- **The session keeps its answered lines.** `Session::answers: Vec<Answered { start, label, outcome, line, len, name, mark, open }>`, `start` being the ask's own marker (kept, not freed), `label`/`outcome`/`line` where the label, the outcome and the line with its time end, `len` the whole text with the sent rows, `name` the rows' name width and `mark` the mark's face. Kept apart from `Row::Ask`, whose markers `end_run` frees.
- **The rows are always in the text, folded by `Invisible`,** as a thought's content is. The length never changes, so the group row's span is set once by `settle_row`, and folding is a properties pass: `render_answered(buffer, at, answered, in_group)` lays the faces and either the rows' decorations or one `Invisible` from the line's end. `Row::Ask`'s render in an open group calls it for a settled line. Alternative: inserting and deleting the rows on `TAB` — would need the group row's length updated on each toggle.
- **The group indent is a line decoration,** `Indent(2)`, not inserted spaces, so a line settled in the run before its group forms needs nothing when the group forms; `render_group` redraws it with the indent.
- **The replaced call stays in the text, under `Invisible`.** A `replaced` flag on `Tool`, set when its question closes; `render_tool` then hides the block and its separator (the newline before it in a group, the blank line after it alone) instead of drawing it, and `hidden` reports it, so `TAB` and the fold skip it. The call keeps updating unseen. Alternative: deleting the call — its later `tool_call_update`s would add it back, and the group's count and run bookkeeping would need unwinding.
- **What was sent** comes from `Question::content`, which now also returns the `(name, answer)` pairs; the summary joins them (one field: the answer; several: ` · `, or `Nothing answered`). `answer_question` keeps them on the question for `settle`.
- **`TAB` order:** after a pending several-questions block and pastes, before tool calls, so an answered line in an open group opens rather than folding the group; one with nothing sent falls through to the group, as a row without content does.

## Risks / Trade-offs

- [The group line still counts the replaced call] → acceptable: it names what the agent did; noted in `docs/editor.md`.
- [An `Invisible` run spanning a blank line at a block boundary] → the fold already hides runs across newlines; a test covers a lone call's line.
