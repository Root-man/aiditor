# Tasks

## 1. The walk

- [x] 1.1 Add `ignore` to `aiditor-services`; `files::index(root)` walks one root with the settings in design.md and returns its sorted relative files or the root's error; `FileOp::Index` / `Msg::Indexed` in core, answered by `files::run`. Verify with services tests for ignored, hidden, `.git`, non-git `.gitignore`, symlinked directory and missing root.

## 2. The index in the core

- [x] 2.1 Store the index per project with a request generation; `project-index` prompts `Project: `, echoes `Indexing <project>...`, sends the op, and on the answer stores it and echoes the count and root errors; drop stale and unknown-project answers. Bind `C-x p i`. Harness carries out `FileOp::Index`. Verify with harness tests for the count, a missing root, a stale answer, and `C-x p` listing `project-index`.
- [x] 2.2 Record the file index in `docs/editor.md` (Subsystems) and verify it matches the spec.

## 3. Checks

- [x] 3.1 `cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace` and the rest of `.github/workflows/ci.yml` pass.
