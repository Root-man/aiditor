# Tasks

## 1. Ops and service

- [x] 1.1 Add `TerminalOp::Write { id, bytes }` and `TerminalOp::Resize { id, rows, cols }` to `aiditor-core`, and verify the workspace builds
- [x] 1.2 Replace `terminal::spawn` with a `Terminals` service: open the PTY on the caller's thread, keep the master and a writer-thread sender per id, give the reader its own fd dup; verify the existing `tests/terminal.rs` pass through it
- [x] 1.3 Route every `Effect::Terminal` in the editor through one `Terminals`, draining the receiver a `Spawn` returns; verify `cargo build -p aiditor`

## 2. Tests

- [x] 2.1 Test that `hi\n` then `\x04` to `cat` echoes `hi` twice and exits 0
- [x] 2.2 Test that after a resize to 30x100, `stty size` prints `30 100`
- [x] 2.3 Test that `\x03` ends `sleep 30` within seconds, not with a clean exit
- [x] 2.4 Test that `aé`, `\x7f`, `\n` to `read -r line` prints `<a>`
- [x] 2.5 Test that a 1 MB write to a raw-mode program that doesn't read returns at once
- [x] 2.6 Run `cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace` and the rest of CI, and verify they pass

## 3. Docs

- [x] 3.1 Record the built behavior in `docs/editor.md`'s Terminal paragraph
