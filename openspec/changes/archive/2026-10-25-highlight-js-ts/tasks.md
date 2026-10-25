# Tasks

## 1. Pipeline

- [x] 1.1 Add `tree-sitter`, `tree-sitter-javascript`, `tree-sitter-typescript` to `aiditor-core`; verify `cargo build -p aiditor-core`
- [x] 1.2 `syntax.rs`: language table by extension, combined queries, `Syntax` with edit, reparse and visible-range highlights; unit test of a snippet's colours and the prefix fallback
- [x] 1.3 `Buffer` holds `Option<Syntax>` set for file buffers; raw edits apply `Tree::edit`; `window_display` reparses stale trees; test typing, undo and revert through the harness
- [x] 1.4 `theme.syntax` in `settings.rs`/`settings.toml`; display builder colours chars from highlights under face properties and the region; tests for override and region precedence
- [x] 1.5 Record the build in `docs/editor.md`'s Language support paragraph
- [x] 1.6 `cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace` pass

## Workflow follow-up

- Archive the change after review.
