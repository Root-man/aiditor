## Why

An answered question leaves a plain dim line that can't be opened, and the question's tool call keeps a row of its own beside it. Frame 6f gives the answered question the shape of a tool-call row: a fold mark, a status mark, the question and the answer, `TAB` opening what was sent (#413).

## What Changes

- A closed question's block becomes `▸ <mark> <label>  <answer>`, with the clock time at the right: `✓` with the answer, `– … Skipped`, `✗ … Cancelled`, `○ … Closed` when the agent withdraws it, or `○ … Closed, the turn ended first` when a non-blocking question's turn ends. The label is the message, or `<n> questions` for a block of several, whose answer reads the answered fields' answers joined by ` · `.
- Marks and faces as a tool call's: `▸` dim, `✓` in `status-running`, `✗` in `status-failed`, `–` and `○` dim, the label plain and the answer dim. In a group it sits two columns in, as a row.
- `TAB` on an accepted question's line opens a row per field that was sent, `<name>  <answer>` on `session-block`, and folds them again.
- The question's tool call shows no row of its own once the question is closed; the line is its row.
- A `session/load` replay shows no question block (tested; the editor already draws none).
- Permission requests keep their lines.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `agent-questions`: the closed question's line, its `TAB` rows, and the tool call row it replaces.

## Impact

- `crates/aiditor-core/src/agents.rs`: closing a question, `TAB`, the session's answered lines.
- `crates/aiditor-core/src/agents/render.rs`, `groups.rs`: drawing the line and its rows, hiding the replaced call.
- `crates/aiditor-core/src/agents/questions.rs`: the label and what was sent.
- `crates/aiditor-services/tests/scenario.rs`: its `○ withdrawn` check.
- `docs/editor.md`: the Questions paragraph.
