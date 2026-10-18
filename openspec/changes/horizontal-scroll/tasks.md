# Tasks

## 1. Horizontal scroll

- [ ] 1.1 Add the offset to `Window` (`hscroll(col, cols)`, kept by `split`, reset by `show`) and verify with a window unit test of the clamp and the reset to 0
- [ ] 1.2 Pass the window's columns to `window_display`, cut point's line and shift the cursor; verify with a harness frame snapshot of `C-e` on a long line (other lines unshifted) and `C-a` back
- [ ] 1.3 Scroll the minibuffer line with the frame's columns less the prompt; verify with a harness test
- [ ] 1.4 Add the offset to a click's column on point's line; verify with a harness click test
- [ ] 1.5 Record the defaults in `docs/editor.md` under Long lines and verify `cargo fmt`, clippy and tests pass

## Workflow follow-up

- Verify, commit, archive and ship per the implement skill.
