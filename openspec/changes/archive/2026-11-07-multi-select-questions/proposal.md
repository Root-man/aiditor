# Proposal

## Why

claude-agent-acp sends `AskUserQuestion`'s multi-select questions as an `array` field with an `anyOf` item enum, and forwards each option's `preview` (mockups, code, comparisons) under `_meta["_claude/askUserQuestionOption"].preview`. Today such a question is declined and previews are dropped. Issue #408 draws both, as frame 6d shows.

## What Changes

- An `array` choice field (items `anyOf` or `enum`) with at most the same companion field as a single choice is a multi-select question. Its rows read `[ ]` or `[x]` after the chip; a digit or `RET` ticks or unticks one, `o` reads an own answer that joins the picks (and unticks it when ticked), and `C-c C-y` with point in its block sends the ticked values as a string array, the own answer in the companion. `minItems` and `maxItems` are not drawn; a send out of range stays open with `Pick at least <n>` or `Pick at most <n>`.
- An option with a preview shows it under its row while point is on that row, in `session-block`: at most 8 lines, else its first 7 and `… <n> more lines`. Each window follows its own point; the lines are drawn, not in the buffer.
- Options past nine get no number chip and the hint names `1-9`; `RET` picks them.
- `C-c C-y` with point in a multi-select question's block sends it; elsewhere it still answers the oldest permission request.

## Capabilities

### New Capabilities

### Modified Capabilities
- `agent-questions`: multi-select questions, option previews, options past nine.
- `session-buffers`: `C-c C-y` sends the multi-select question at point.

## Impact

- `crates/aiditor-core/src/agents/questions.rs`, `agents.rs`, `frame.rs`.
- `docs/editor.md`.
- No new dependencies.
