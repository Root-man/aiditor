# Tasks

## 1. Questions

- [x] 1.1 Read `_meta.codex.delivery: "async"` into `Question::nonblocking`, with frame 6f's hint for a single choice; verify with a `questions.rs` test

## 2. Session

- [x] 2.1 Non-blocking questions leave `activity` and the sessions list's state alone, and the mode line counts them; verify with a core test
- [x] 2.2 A non-blocking question never moves point, output goes in below it, and the turn's end closes it as `○ withdrawn`; verify with core tests

## 3. Docs and checks

- [x] 3.1 Record non-blocking questions in `docs/editor.md`
- [x] 3.2 `cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace` pass
