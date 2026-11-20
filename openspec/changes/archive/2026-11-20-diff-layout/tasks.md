# Tasks

## 1. Core

- [x] 1.1 `Windows.saved` and `windows_mut` over it; `diff.rs` with `diff-status`, `diff-refresh`, `diff-quit`, the list kind, rendering and `Msg::GitStatus` routing
- [x] 1.2 `diff-list-mode` and `diff-view` keymaps, the layer in `bindings.rs`, commands in `builtins.rs`, echo-line hints
- [x] 1.3 Faces `diff-file-*` and `diff-list-group`
- [x] 1.4 `FrameWindow.bar`: no mode line for the list, one spanning bar for its right neighbour

## 2. Frontend

- [x] 2.1 `element.rs` paints each window's bar where the core puts it, or none

## 3. Tests and docs

- [x] 3.1 Core tests: rows (two roots, branches, both groups, clean, non-git), layout and restore, select again, refresh keeps point, one bar
- [x] 3.2 `docs/editor.md`: the view as built
