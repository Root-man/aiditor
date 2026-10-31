# Proposal

## Why

A mode line is one string in one colour, so it can't show the design's two groups, its `│` separators or colour for state. The file, terminal and session mode line tasks (epic "Mode lines") all need segments to build on.

## What Changes

- A buffer's mode line becomes a left group (what the buffer is) and a right group (where point is, what is running), each a list of segments that exist only when they apply.
- Segments are split by `│` in the `separator` face and the right group is drawn against the window's right inset.
- Today's text stays and moves onto segments: `(DOS)` goes right, `[deleted]`, `[conflict]` and the agent mode become left segments, the terminal's input mode and `[unwatched]` go right.
- State gets the design's colours. Inactive windows keep the layout and drop every colour and all bold.
- A mode line wider than its window is cut with `…`, using the workspace modeline's rule for sharing the room.

Not here: the file, terminal and session contents the later tasks add (paths, branch, `●`, `char` without brackets, mode colours by risk).

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `display-model`: the mode line's groups, segments, colours and truncation.
- `frontend-rendering`: drawing the two groups, and inactive mode lines dropping segment colour.
- `terminal-buffers`: the input mode moves to the right group.
- `session-buffers`: the agent mode becomes a left segment.

## Impact

- `aiditor-core`: `display.rs` (the model and per-buffer-type segments), `frame.rs` (`[unwatched]`, fitting, shared room rule), tests and snapshots that read `mode_line`, including `aiditor-services/tests/agents.rs`.
- `aiditor`: `element.rs` draws two shaped lines.
- `docs/editor.md` records the segments.
