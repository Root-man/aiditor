# Tasks

## 1. Core

- [x] 1.1 Add a modification tick to `Buffer`, bumped on every raw insert and delete
- [x] 1.2 Give `Window` an active flag and a mark ring of markers (push, pop, exchange, deactivate; at most 16; dropped by `show`), with unit tests
- [x] 1.3 Add `set-mark-command` (set, toggle on repeat, `C-u` pops) and `exchange-point-and-mark`; make `keyboard-quit` deactivate the mark
- [x] 1.4 Deactivate the mark in `Ctx::call` when the current buffer's tick changed
- [x] 1.5 Push the mark in `beginning-of-buffer`/`end-of-buffer` unless active or plain `C-u`; update the motion tests
- [x] 1.6 Pass the mark to the display only while active; update the display region test

## 2. Keys

- [x] 2.1 Bind `C-SPC` and `C-x C-x` in the bundled keymap
- [x] 2.2 Map Ctrl+Space to `C-SPC` in the frontend's `to_keys`, with a test

## 3. Tests and docs

- [x] 3.1 Harness tests for every scenario in the `mark`, `keymaps` and `motion` deltas, including a display snapshot that shows the region and loses it on `C-g`
- [x] 3.2 Record the decision in `docs/editor.md` (Commands and keymaps), replacing the #27 note on `M-<`/`M->`, and note the macOS Ctrl+Space conflict
