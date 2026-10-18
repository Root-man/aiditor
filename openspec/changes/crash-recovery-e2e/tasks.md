# Tasks

## 1. Headless binary

- [x] 1.1 Add `crates/aiditor-services/src/bin/aiditor-headless.rs`: `RECOVERY_DIR FILE SCRIPT`; open FILE, send `Msg::Recover(recovery::list(dir))`, feed the script (plain chars as `Msg::Insert`, as the harness does), carry out `Save`, `Open`, `ListDir` synchronously through `files` and recovery effects through `recovery::spawn`, carrying out each message's effects before the next key, then fire `Msg::Tick` at each `next_deadline` on the real clock; on `Effect::Quit` join the recovery thread and exit 0. Verify `cargo build -p aiditor-services --bins` succeeds.

## 2. End-to-end test

- [x] 2.1 Add `crates/aiditor-services/tests/crash_recovery.rs`: run 1 types an edit, poll until a recovery file (no extension) appears, SIGKILL, assert the file on disk is unchanged; run 2 `RET C-x C-s C-x C-c` exits 0 within a timeout, the file holds the edit and the recovery dir is empty. Verify `cargo test -p aiditor-services --test crash_recovery` passes.
- [x] 2.2 Note the end-to-end test in `docs/editor.md` Crash safety and verify `cargo fmt --all --check`, `cargo clippy --all-targets -- -D warnings` and `cargo test` pass.

## Workflow follow-up

- Archive the change after the PR merges.
