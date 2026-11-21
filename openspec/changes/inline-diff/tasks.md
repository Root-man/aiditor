# Tasks

## 1. Core

- [x] 1.1 Keep the diff window id and shown diff in `DiffList`; `sync_diff` after each message shows the file at point
- [x] 1.2 Handle `GitDiffed`, view-owned `Opened` and `LinesDiffed`; side buffers for the index text and reasons
- [x] 1.3 Build the diff rows and the added ranges; the 300 ms re-diff timer on tick changes, dropping stale answers
- [x] 1.4 Commands: next/previous change and file, copy removed lines, `diff-visit`; bindings in `keymap.toml`; skip `diff-view` in the minibuffer
- [x] 1.5 Added-line bars and mode line segments in `frame()`

## 2. Tests

- [x] 2.1 Core tests: layout of rows and marks, staged vs changes, edit re-diff and stale drop, binary reason, untracked and deleted, motion and messages, copy, `C-g` quit, mode line, revert follows

## 3. Docs

- [x] 3.1 `docs/editor.md`: the inline diff as built
