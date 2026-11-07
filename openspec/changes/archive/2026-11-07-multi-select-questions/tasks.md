# Tasks

## 1. Questions

- [x] 1.1 Parse `array` choice fields with `anyOf` or `enum` items and their `minItems`/`maxItems` as multi-select questions; read each option's preview; verify with `questions.rs` tests for the Claude shape and the bare enum
- [x] 1.2 Rows `[ ]`/`[x]`, the multi-select hint, the preview hint, blank chips past nine; verify with `questions.rs` tests on the lines
- [x] 1.3 Multi-select content and the answered line; range checks; verify with `questions.rs` tests

## 2. Session

- [x] 2.1 Digits, `RET` and `o` tick in a multi-select block, redrawing it in place; `C-c C-y` at point sends, refusing out-of-range sends with the echo line; verify with core tests
- [x] 2.2 A tenth option picked with `RET`; verify with a core test

## 3. Frame

- [x] 3.1 Splice the preview of the option at point under its row, folded past 8 lines; verify with a frame test that moves point between options

## 4. Docs and checks

- [x] 4.1 Record multi-select, previews and options past nine in `docs/editor.md`
- [x] 4.2 `cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace` pass
