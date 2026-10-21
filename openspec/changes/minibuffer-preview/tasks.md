# Tasks

## 1. Preview hook

- [x] 1.1 Add `preview: Option<fn(&Core, &str) -> Option<BufferId>>` to `ArgSpec`; `cargo build` passes
- [x] 1.2 Add `saved` and `previewed` to `Minibuffer`, `Core::preview` (run in `handle` after `poll_completion`) and the restore in `close_minibuffer`; a harness test with a previewing test command checks that `C-n` previews, no match restores, and preview then `C-g` leaves the frame display, point, mark, active mark and mark ring exactly as before, and that `RET` runs the command from the restored window

## 2. Buffer switching

- [x] 2.1 Give `switch-to-buffer` the hook (`find_here`); harness tests: `C-x b` shows the previous buffer at once, `C-x b C-n C-g` leaves the window and the buffer order as before, and `C-x 4 b` does not preview
- [x] 2.2 Record what was built in the Minibuffer and Buffers bullets of `docs/editor.md`

## Workflow follow-up

- Archive the change after review.
