# Proposal

## Why

A session transcript reads as one blob: only the `❯` tells a prompt from the agent's output, and nothing marks where one turn ends and the next begins. The mode line's model, effort and `ctx` are all one dim grey, so a costly model, a low effort or a nearly full context don't catch the eye. Frames 6 and 6g of the canvas now settle how to fix both, and times to the second were asked for.

## What Changes

- A finished turn's `Worked <duration>` line is drawn as a rule: a 1 px line through the row's empty space, around the label and its clock time (frame 6g, option F).
- A sent or replayed prompt keeps its `session-user` band and gains a 1 px rule above its first line and below its last, with its text bold in a brighter colour (frame 6g, option D).
- Message and turn-end clock times read `HH:MM:SS`. The workspace clock stays `HH:MM`.
- **BREAKING (internal)**: `Msg::Clock` carries the local time of day as a `Duration` since midnight instead of `HH:MM` text; the core formats times itself.
- `ctx N%` is coloured as a whole: `status-running` under 50%, `status-waiting` from 50% to under 80%, `status-failed` from 80%.
- The session model and effort are coloured by item quality, in seven new faces `quality-poor` … `quality-artifact`: Fable and Astra legendary, Opus and Sol epic, Sonnet and Terra rare, Haiku and Luna uncommon; effort `low` common, `medium` uncommon, `high` rare, `xhigh` epic, `max` legendary, `ultra` artifact, `default` poor. Unknown names stay `mode-line-dim`. Inactive mode lines still drop colour.
- One new face for the prompt band's rules, `session-user-rule`; `session-user` gains the text's foreground and bold.

Not included: Gruvbox theme tables (the bundled theme is still the default one), the colours in frames 9, 10 and 12, and the running `Working for` line, which stays plain until the turn ends.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `session-buffers`: the working line settles into a rule; prompts gain rules and a text face; messages show `HH:MM:SS`; model, effort and context usage take colours.
- `display-model`: lines can carry a horizontal rule decoration and top and bottom border decorations; session model, effort and context usage leave the `mode-line-dim` rule.
- `frontend-rendering`: the core gets the time of day, not `HH:MM` text; the new decorations are drawn.
- `settings`: `themes.default` defines the eight new faces, and `session-user` a foreground and bold.

## Impact

- `crates/aiditor-core`: `lib.rs` (`Msg::Clock`, `Core::clock`), `agents.rs` (mode line, `settle_turn`, stamps), `agents/render.rs` (echo faces and borders, the turn rule), `buffer.rs` (`Decoration`), `display.rs`, `settings.rs` and `settings.toml` (faces).
- `crates/aiditor-services/src/clock.rs`: also returns the time since midnight.
- `crates/aiditor/src/editor.rs` (sends the new clock) and `element.rs` (draws the new decorations).
- Tests that send `Msg::Clock("14:01")` or assert `HH:MM` stamps change.
- `docs/editor.md`: the times, working timer, session mode line and faces sections.
