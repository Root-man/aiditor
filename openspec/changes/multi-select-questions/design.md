# Design

## Context

`agents/questions.rs` parses a single-choice question and draws its block; `agents.rs` keeps it in `Session::asks` and answers it. The block's text has a fixed length: its `Row::Ask` length in the tool group and `Ask::len` assume so. The frame builder already adds `Background(region)` to the option row at point (`Session::question_row`).

## Decisions

1. **Multi-select is a field of `Question`.** `Question` gains `multi: Option<Multi>` holding the ticks, the own answer's text and `minItems`/`maxItems`. Parsing accepts an `Array` field with `anyOf` (titled) or `enum` items as the choice; companions are found the same way. Alternative, a separate type: duplicates rows, faces and answers.

2. **Ticking rewrites the block in place.** `[ ]` and `[x]` have the same length, so a tick replaces each changed char in place (`rewrite` would move point on that line to its start) and draws the block again; `Ask::len` and the group row stay right. The own row reads `[x] Own answer` once one is set; its text shows in the answered line, not the row, so the length holds.

3. **Sending.** `C-c C-y` (`agent-allow-once`) first checks for a multi-select question at point and sends it; otherwise it behaves as before. Point-only, so a stray `C-c C-y` in the prompt never sends a half-ticked question. Content: `{<key>: [values…]}` plus `{<key>_custom: text}` for a Claude own answer (the adapter joins it to the picks); for a codex `user_note` companion, `None of the above` joins the values and the note carries the text. The count checked against `minItems`/`maxItems` includes the own answer.

4. **Previews are display lines.** The frame builder asks the session for the preview of the option at the window's point and splices its lines in after that row's last display line, then truncates to the rows the window had. They carry the block's `Indent(1)` and `Background(permission)`, start under the option's title, and draw the text on `session-block`, padded to the longest preview line. In the rope instead, every preview would have to be hidden and shown as point moves, `C-n` would step into them, and two windows couldn't preview different options.

5. **Options past nine** get a blank chip, and the hint's keys stop at `1-9`.

## Risks / Trade-offs

- [Preview lines aren't in the window's scroll arithmetic] → a row near the window's bottom shows its preview cut; `shortcut:` comment, upgrade by scrolling with the preview's rows if it bites.
- [A click on a preview line maps to the rows below it] → rare; same comment.
- [Long preview lines are cut at the window's width, not wrapped] → previews are mockups and code, where wrapping would mislead anyway.
