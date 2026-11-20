# Tasks

## 1. Types and ranges

- [x] 1.1 Add `GitSide`, `GitObjects`, `FileDiff`, `Hunk`, `ChangedFile`, `FileOp::{GitDiff, DiffLines, GitDiffFiles}` and `Msg::{GitDiffed, LinesDiffed, GitDiffFiles}` to `crates/aiditor-core/src/lib.rs`, no-op core arms, and the harness leaving the ops as effects; verify `cargo build --workspace`
- [x] 1.2 Add `buffer::hunks(old, new)` merging `similar`'s adjacent changed line ops; verify with a unit test

## 2. Service operations

- [x] 2.1 In `crates/aiditor-services/src/files.rs`, add a shared `git(dir)` command with the safety flags used by status and `changed()`, and a shared status letter parser
- [x] 2.2 Implement `GitDiff` (sides read with `cat-file` or from disk, decoded, ranged) and `DiffLines`; verify with temp-dir repository tests for every scenario of the first three requirements
- [x] 2.3 Implement `GitDiffFiles` with a `--name-status -z` parser, objects env on both git operations and the `-` refusal; verify with temp-dir tests for the listing, snapshot tree and safety scenarios

## 3. Docs and checks

- [x] 3.1 Record the operations in `docs/editor.md`'s Git subsystem
- [x] 3.2 `cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace` pass
