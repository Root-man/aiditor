## Why

Issue #30 is the open question "Mouse (selection, scrolling, clicks) and syncing the kill ring with the system clipboard". The kill ring (#31) and feeding mouse events to the core (#12) are blocked on it, and the spec leaves open what `s-c`, `s-v` and `s-x` do.

## What Changes

- Record the decision in `docs/editor.md` under Commands and keymaps:
  - one kill ring shared with the system clipboard, using Emacs's defaults: a kill also writes the clipboard; a yank first pushes clipboard text the editor did not write onto the ring; foreign clipboard text is saved to the ring before a kill overwrites it;
  - `s-c`, `s-x` and `s-v` run `kill-ring-save`, `kill-region` and `yank`;
  - plain text only, no Linux PRIMARY selection, a ring of 60 entries;
  - the MVP mouse: the wheel or trackpad scrolls the window under the pointer, keeping point visible as `C-v` does; a left click selects the window, moves point to the clicked char and deactivates the mark. Drag, double and triple click, shift-click, a context menu and mode-line clicks come later.
- Remove the question from Open questions.
- Update the `s-` key note under "Keys from the frontend", which defers to this decision.

No code changes: the behavior is built and specified in #31 and #12.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

None. This change only records a decision; `skip_specs` is set and the behavior's specs arrive with #31 and #12.

## Impact

- `docs/editor.md` only.
