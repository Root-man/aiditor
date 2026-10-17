# Tasks

## 1. Window

- [x] 1.1 `Window` keeps the rows from the last `scroll` call and lets a command set its first line; existing window tests pass

## 2. Commands

- [x] 2.1 Char, line (with goal column), word and line-end commands as builtins; key-script harness tests for each `motion` scenario of those requirements, including `C-u 3 C-f`, pass
- [x] 2.2 Buffer-end, scroll and recenter commands; harness tests for their scenarios pass
- [x] 2.3 Bind every motion key and the arrows in `keymap.toml`, delete the fallback arrow motion in `Ctx::key`, and update the arrow-key harness tests to the new end-of-buffer behaviour; `cargo test --workspace` passes

## 3. Checks and docs

- [x] 3.1 Record in `docs/editor.md` (Commands and keymaps) the decisions from #27: Emacs command names, arrows bound to them, ends reported, no mark on `M-<`/`M->`, char goal column
- [x] 3.2 `cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D warnings` and the rest of CI pass
