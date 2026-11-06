# Tasks

## 1. Char motion

- [x] 1.1 `forward_char` skips invisible runs with no placeholder and crosses a fold or display range in one step; verify with a `motion.rs` harness test over a link and a fold

## 2. Docs and checks

- [x] 2.1 Update `docs/editor.md` on which motion can land inside a fold; verify by reading the paragraph
- [x] 2.2 `cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace` pass
