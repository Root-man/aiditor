# Tasks

## 1. Opening

- [x] 1.1 `Terminal::resize(rows, cols)` and its size
- [x] 1.2 Window cell sizes helper from `Tree::layout` and the frame size
- [x] 1.3 `vterm` command: name `vterm<n><workspace>`, working directory, show in the selected window, `TerminalOp::Spawn` at the window's size

## 2. Life and size

- [x] 2.1 `TerminalExited` and failed `TerminalSpawned` drop the entry and kill the buffer; failure echoes the reason
- [x] 2.2 Post-message pass: `Kill` for terminals whose buffer is gone, resize grids and send `Resize` when the smallest window's size changes

## 3. Queries

- [x] 3.1 `kill-buffer` asks `Buffer <name> has a running process; kill it? ` for a live terminal
- [x] 3.2 `save_some` returns the saves instead of starting them on go-ahead; callers start them
- [x] 3.3 Quitting asks `Active processes exist; kill them and exit anyway? ` after the save questions

## 4. Tests and docs

- [x] 4.1 Harness records terminal ops; core tests for every scenario in the delta specs
- [x] 4.2 Record what was built in `docs/editor.md`
