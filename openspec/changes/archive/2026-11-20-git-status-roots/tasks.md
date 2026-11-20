# Tasks

## 1. Types and operation

- [x] 1.1 Add `GitStatus`, `GitFile`, `GitChange`, `FileOp::GitStatus` and `Msg::GitStatus` to `crates/aiditor-core/src/lib.rs`, a no-op core arm and the harness passing the op through as an effect; verify `cargo build --workspace`
- [x] 1.2 In `crates/aiditor-services/src/files.rs`, split `head()` so a directory's repository root, `HEAD` path and branch come from one helper, and share the repository-filter check with `changed()`; verify the existing `files.rs` and `file_sync` tests pass
- [x] 1.3 Implement the operation: one scoped thread per root, `git status --porcelain=v2 -z --untracked-files=all` limited to the root with the same safety flags as `changed()`, parsed into `GitFile`s; verify with service tests in temp-dir repositories for every scenario in `specs/git-status/spec.md`

## 2. Docs and checks

- [x] 2.1 Record the operation in `docs/editor.md`'s Git subsystem; verify the line is there
- [x] 2.2 `cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace` pass
