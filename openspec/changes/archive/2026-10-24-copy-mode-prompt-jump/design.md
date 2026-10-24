# Design

## Context

`Terminal::marks` holds `(absolute line, Mark)`. Copy mode's text is built once on entry by `Terminal::copy_text`, which already maps the cursor's grid point to a text line and char column across joined wrapped rows. Output keeps feeding the grid while copy mode's text stays frozen, so grid lines can't be mapped later.

## Goals / Non-Goals

**Goals:** jumps that match the frozen text; wrapped commands land on the right char.

**Non-Goals:** jumps in char mode (vterm binds them in `vterm-mode` too; the issue asks for copy mode); per-command output capture; tracking marks through column reflow.

## Decisions

- **Marks keep `(line, column, Mark)`.** `B` arrives after the prompt's text on the same line, so the line alone would land on the prompt. The column is the grid column, turned into chars like the cursor's (wide-char spacers skipped).
- **Targets are computed in `copy_text`,** in the same pass that maps the cursor: marks are grouped by absolute line, each mapped to (text line, char column) when its row is visited. Marks on dropped blank rows get no place. Then, in arrival order, an `A` starts a command and a `B` after it replaces its target; a `B` with no `A` before it is its own target. Targets are sorted and deduplicated. Alternative: map on each jump from the grid — wrong once output arrives during copy mode.
- **The buffer keeps the targets as char indices** beside copy mode's text; leaving copy mode clears them. The text is read-only and has no undo, so the indices stay valid.
- **Count.** The nth target after (or before) point, stopping at the farthest when fewer exist; `No prompt found` when there is none in that direction, or the count is 0.

## Risks / Trade-offs

- [Marks dropped by a full scrollback or ED 3] → no targets, so the jump echoes `No prompt found`.
- [Reflow shifts marks] → the existing `shortcut:` stands.
