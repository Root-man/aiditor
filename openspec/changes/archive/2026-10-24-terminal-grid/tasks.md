# Tasks

## 1. Emulator

- [x] 1.1 Add `alacritty_terminal` 0.26 to `aiditor-core` without default features; verify `cargo tree -p aiditor-core` has no `gpui` or `tokio`
- [x] 1.2 Add `terminal::Terminal` (`new`, `feed`, `screen`) with replies collected from `PtyWrite`; unit tests for plain lines, cursor moves and clears, scrolling, a split escape sequence, `\x1b[6n` and ignored title/bell/clipboard

## 2. Terminal buffers

- [x] 2.1 Add `Content::Terminal`, an empty `text()` for it and refused raw edits; verify the existing buffer tests pass
- [x] 2.2 Add `Core::create_terminal` and the id map, and feed `Msg::TerminalOutput` into the buffer's grid, sending replies as `TerminalOp::Write`; harness tests for the screen, the `6n` reply and unknown output
- [x] 2.3 Harness test that motion, `C-x h` and swiper don't panic in a terminal buffer and edits echo `Buffer is read-only`
- [x] 2.4 Run `cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace` and the rest of CI, and verify they pass

## 3. Docs

- [x] 3.1 Record the built behavior in `docs/editor.md`'s Output floods section
