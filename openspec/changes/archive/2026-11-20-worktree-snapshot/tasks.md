# Tasks

## 1. Types

- [x] 1.1 Add `Snapshot`, `FileOp::GitSnapshot` and `Msg::GitSnapshot` to `crates/aiditor-core/src/lib.rs`, a no-op core arm and the harness leaving the op as an effect; verify `cargo build --workspace`

## 2. Service

- [x] 2.1 Implement `files::snapshot(root, dir)` (seed copy, `git add -A`, `git write-tree`, rename over the kept index) and the `GitSnapshot` arm; verify with temp-dir tests for every kind of change, not a repository, a linked worktree, the repository untouched and the stat cache
- [x] 2.2 Implement `files::clear_snapshots(parent)` and call it on a thread at launch in `crates/aiditor/src/main.rs`; verify with a test of a live and a dead pid's directory

## 3. Benchmark and docs

- [x] 3.1 Add `snapshot/300-changed-files-{first,again}` to `crates/aiditor-services/benches/services.rs` and its rows to `docs/performance.md`; run it
- [x] 3.2 Record the operation and the measured times in `docs/editor.md`'s Git subsystem
- [x] 3.3 `cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace` pass
