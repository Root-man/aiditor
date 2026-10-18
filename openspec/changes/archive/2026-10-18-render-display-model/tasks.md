# Tasks

## 1. Core

- [x] 1.1 Add `mode_line` and `mode_line_inactive` to `Theme` and `settings.toml`; verify a settings test that the user's `mode_line` overrides and the inactive default stays
- [x] 1.2 Give `Tree::layout` a cell size and snap split edges to whole lines and cells (no snap for a zero cell); drop its `shortcut:` comment; verify the window.rs unit test for 3, 2 and 5 lines in a 10-line frame
- [x] 1.3 Add `Msg::Resize { width, height, cell_width }`, store it in `Core`, make `Core::frame()` use it, add `id` to `FrameWindow`; add a harness `frame(w, h)` helper; verify the split snapshots (updated) and a resize test
- [x] 1.4 Make `Msg::Click { window, line, col }` select the window and move point, and `Msg::Scroll { window, lines }` scroll that window without changing the selection; update the existing click and scroll harness tests; verify new tests for clicking and scrolling the unselected window

## 2. Frontend

- [x] 2.1 Replace the div drawing with a `FrameElement` that reports `Resize`, shapes each display line whole with one run per span plus marked text, paints decorations, text, the cursor quad and the mode lines per window under a content mask, and records the layout and shaping time
- [x] 2.2 Route clicks and wheel scrolls through the recorded layout to the window under the pointer; answer `bounds_for_range` from the recorded cursor; drop `text_bounds`, `top` and the old `shortcut:` comments
- [x] 2.3 Always draw the echo area; lay the hint out in columns above it; add the shaping time to the `--debug` stats line
- [x] 2.4 Verify with GPUI tests: the cursor sits at `x_for_index` of point's column and point's line is shaped whole; clicking the right window of a `C-x 3` split selects it and moves its point; the existing mouse test passes on the new layout

## 3. Docs

- [x] 3.1 Note in `docs/editor.md` that the frontend draws the frame display; verify by reading the edited lines

## Workflow follow-up

- Manual rendering checklist in the PR; verify, commit, archive and ship per the implement skill.
