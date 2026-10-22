# Tasks

## 1. The search

- [x] 1.1 Add `grep-searcher`, `grep-regex`, `grep-matcher` to `aiditor-services`; extract `files::walk(root)` and use it in `files::index` (the index tests still pass).
- [x] 1.2 `GrepOp`, `Hit` and `Effect::Grep` / `Msg::Grepped` in core (the core ignores `Grepped`, the harness ignores `Effect::Grep`); `grep::search` with cancel check and send callback, `grep::Grep` with the latest-search number. Verify with services tests for ignored and binary files, root order, smart case, invalid pattern, hit fields (column in chars), limit, done, cancel after the first batch stopping within a file, and a superseded search sending nothing more.

## 2. Wiring and docs

- [x] 2.1 The app runs `Effect::Grep` through a `Grep` handle on the blocking pool; `aiditor-headless` runs it inline. Verify the workspace builds.
- [x] 2.2 Record the grep service in `docs/editor.md` (Subsystems) and verify it matches the spec.

## 3. Checks

- [x] 3.1 `cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace` and the rest of `.github/workflows/ci.yml` pass.

## Workflow follow-up

- Archive the change after review.
