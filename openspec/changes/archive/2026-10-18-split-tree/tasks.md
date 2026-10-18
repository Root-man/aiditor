# Tasks

## 1. Split tree

- [x] 1.1 Add `WindowId`, `Tree` (split, remove, leaves in order, layout into `Rect`) to `window.rs` and `Rect` to `display.rs`, plus `Window::release` used by `show`; verify unit tests for repeated splits (25/25/50), removing a leaf and side-by-side layout

## 2. Core and commands

- [x] 2.1 Add `Core::selected`, `Core::others` and `Core::tree`; make `Core::open` show in the selected window; implement `split-window-below`, `split-window-right`, `other-window`, `delete-window`, `delete-other-windows` and bind `C-x 2`, `C-x 3`, `C-x o`, `C-x 0`, `C-x 1`; verify harness tests for every `windows` delta scenario
- [x] 2.2 Add `FrameDisplay` and `Core::frame(width, height)`; verify insta snapshots of a stacked and a side-by-side split on a two-line buffer

## 3. Docs

- [x] 3.1 Note in `docs/editor.md` what the split tree does now and that the frontend draws it from #11; verify by reading the edited lines

## Workflow follow-up

- Verify, commit, archive and ship per the implement skill.
