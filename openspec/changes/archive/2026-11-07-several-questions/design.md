# Design

## Context

`agents/questions.rs` parses one choice field (`Question`) and draws its block; `agents.rs` keeps it in `Session::asks`, answers it, and rewrites a multi-select's block char for char on a tick, since its length never changes. The frame builder asks `Session::question_row` for the option row at point and its preview.

## Decisions

1. **`Question` is the block, `Field` one choice.** Today's per-field data (title, key, options, own, multi) moves to `Field`; `Question` holds the message, its fields and the open one, plus each single-choice field's recorded answer. One field draws exactly as before, so frames 6d and the existing tests hold. Alternative, a list of `Question`s in the `Ask`: every caller of `ask.question()` would need to pick a field.

2. **Lines of several fields, frame 6e.** ` Questions  <k> of <n>` (`Questions` in `status-waiting` bold), then per field ` ▸ ✓ <title>  <answer>` / ` ▸ ○ <title>  <description>` (marks and the dim text in `session-thought`, `✓` in `status-running`) or, for the open one, ` ▾ ● <title>  <description>` (`●` in `status-waiting`) followed by its option rows three columns further in; titles are padded to the longest. The hint is ` <keys> answer|tick  TAB next  C-c C-y send all  C-c C-k skip all`. A field without a title is named by its key; the description falls back to nothing. Codex puts the question in `title` and the header in `description`, so its rows read the other way round; acceptable until #410 looks at forms.

3. **Redrawing a block whose length changes.** Opening another field changes the block's length, so the block's text is rewritten whole (`rewrite`), `Ask::len` and its `Row::Ask` length updated (`settle_row`, which also redraws a folded group), the properties drawn again, and point moved to the open field's first option in the selected window, where the key was typed. A tick keeps its char-for-char path so point stays.

4. **Answers.** A single-choice field's digit, `RET` or own answer records it and opens the next field; past the last, `open` is `None` and every field folds until `TAB` opens the first. `C-c C-y` with point in a block of several sends the content of each answered field merged, a multi-select counted as answered once something is ticked; a ticked field out of its `minItems`/`maxItems` refuses with `<title>: Pick at least <n>`. The answered line is `✓ <message>  <title>: <answer>; …`, or `Nothing answered`, until #413 shapes it.

5. **Pairing and order.** A companion pairs with the choice whose key it extends with `_custom`, or whose key its `_meta.codex.questionId` names; with one choice any codex note pairs, as before. Fields sort by key with a trailing number compared numerically (`question_2` before `question_10`); codex ids sort by name, as the schema's map has lost the agent's order.

## Risks / Trade-offs

- [Codex's `required` isn't enforced] → a send leaving a codex question out reaches codex without its answer; codex-acp then answers it empty. Revisit with forms (#410).
- [The open field's preview hint isn't shown] → the frame's hint has no room; previews still draw under the row at point.
