# Tasks

## 1. State

- [x] 1.1 Add `Core::session_state` in `agents.rs` returning `[ended]`/`[starting]`/`[waiting]`/`[working]`/`[idle]` for an open session, `None` otherwise; verify through the sessions tests below

## 2. Sessions list

- [x] 2.1 Label rows with the state instead of `[open]` and keep the rendered labels in `SessionList`; verify the updated `open_sessions_are_marked_and_ret_shows_them` test shows `[idle]`
- [x] 2.2 After `sync_agents` in `Core::update`, re-render live lists whose labels changed; verify a new test walking idle → working → waiting → working → idle → ended → none without `g`

## 3. Checks

- [x] 3.1 `cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D warnings` and `cargo test --workspace` pass
