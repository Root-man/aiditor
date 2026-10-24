# Tasks

## 1. Marks and cwd in the core terminal

- [x] 1.1 Add `libc` to `aiditor-core`'s dependencies and verify `cargo build -p aiditor-core` succeeds
- [x] 1.2 Add the `Scan` performer and split `Terminal::feed` around OSC 133, OSC 7, CSI 3 J and ESC c; record marks (`Mark`, `MarkKind`) with `marks()` and the cwd with `cwd()`; verify existing terminal tests still pass
- [x] 1.3 Add core tests for each scenario in the terminal-buffers delta (A/B/C/D lines and exit code, BEL/ST, split batches, alternate screen, clear and RIS, full scrollback, local/this-machine/remote/relative/garbage OSC 7) and verify they pass
- [x] 1.4 Run the core benchmark (`cargo bench -p aiditor-core`) before and after and verify the 64 KB batches parse in about the same time
- [x] 1.5 Record what was built in the Terminal paragraph of `docs/editor.md`, including the benchmark numbers and limits

## Workflow follow-up

- Archive the change after review.
