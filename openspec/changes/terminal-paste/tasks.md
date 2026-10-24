# Tasks

## 1. Paste

- [x] 1.1 Add `Terminal::paste` per the spec; verify a unit test of bracketed and plain bytes, ESC removed
- [x] 1.2 Add `vterm-yank` (clipboard-reading) and bind `s-v` in `[vterm-mode]`; verify harness tests that `s-v` writes foreign clipboard text, bracketed after `\x1b[?2004h`, and echoes `Kill ring is empty` with nothing to paste
- [x] 1.3 Record paste as built in `docs/editor.md`; run fmt, clippy and tests as CI does
