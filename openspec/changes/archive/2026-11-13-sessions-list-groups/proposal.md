## Why

`*Sessions*` is one flat list, newest first, with a header line naming its keys. Frame 14 and the rework decided in #469 group it by activity so today's work is on top and old sessions stay out of the way, in the workspace list's idiom (keys in the echo line, counts in the mode line). Issue #482 is the first step; faces, pins, delete and archive follow in their own tasks.

## What Changes

- **BREAKING** (text): no header line; the keys show in the echo line as the workspace list's do (`n/p move  RET switch  TAB fold  g refresh  q quit`).
- Rows go under headings Pinned, Today, Yesterday, This week (calendar week from Monday), Older and Archived, each `▾ <name> <count>` or `▸ …` when folded, a blank line between groups, empty groups left out.
- Rows read title, agent, time, state, in frame 14's column order. Times by age: `HH:MM` today and yesterday, `Wed 17:30` this week, `YYYY-MM-DD` before.
- `TAB` on a heading folds or unfolds its group. Every group starts unfolded but Archived; folds survive answers and state changes but reset on `g` and a new `agent-sessions`.
- `n`/`p` move over headings and session rows; point keeps its session (or heading) across rebuilds and folds, and goes to the heading when its session is folded away.
- The mode line reads `*Sessions*` │ `<project> · <n> sessions · <m> open`, and on the right the position among the shown session rows.
- Nothing feeds Pinned or Archived yet: pins come with the pins task and archived sessions with #481.

## Capabilities

### New Capabilities

### Modified Capabilities
- `sessions-list`: the buffer's layout (no header, groups, row columns, times by age), folding with `TAB`, moving over headings, point across rebuilds and folds, the mode line and the echo line keys.

## Impact

- `crates/aiditor-core/src/sessions.rs`: rendering, groups, folds, moves, `TAB`, mode line.
- `crates/aiditor-core/src/frame.rs`: echo line hints and mode line for the sessions buffer.
- `crates/aiditor-core/src/lib.rs`: a wall clock the core can read (fixed in tests).
- `crates/aiditor-core/keymap.toml`, `builtins.rs`: `agent-sessions-toggle` on `TAB`.
- `docs/editor.md`: the rework paragraph records what #482 built.
