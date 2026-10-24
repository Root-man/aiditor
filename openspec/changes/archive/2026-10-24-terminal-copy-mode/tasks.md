# Tasks

## 1. Copy text

- [x] 1.1 `Terminal::copy_text` builds scrollback then screen, joining soft-wrapped rows, with the cursor's char offset; unit tests for `seq`-like scrollback, a wrapped line and the alternate screen pass
- [x] 1.2 `Content::Terminal` holds an optional copy rope that `text()` returns, set by a buffer method that clamps markers and records no undo; existing terminal tests still pass

## 2. Copy mode

- [x] 2.1 `vterm-copy-mode` and `vterm-copy-mode-done` commands, the `vterm-copy-mode` minor mode, `C-c C-t` and `RET` bindings, `char_mode()` off in copy mode, the grid drawn only outside it; harness tests cover entering, motion/swiper/`M-w`, inserting failing, output not changing the text, `RET` copying region or line, and toggling off
- [x] 2.2 Record the copy-mode decisions in the Terminal section of `docs/editor.md`
