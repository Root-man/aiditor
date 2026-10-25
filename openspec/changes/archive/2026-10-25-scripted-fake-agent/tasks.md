# Tasks

## 1. Fake agent

- [x] 1.1 Rename `aiditor-test-agent` to `aiditor-fake-agent`, load the `AIDITOR_FAKE_AGENT` JSON script (exit 2 on a bad one) and run `session` steps after `session/new`; verify `cargo build -p aiditor-services` succeeds
- [x] 1.2 Handle `session/prompt` (n-th turn, `end_turn` past the end), `session/set_mode` with `current_mode_update`, and `session/cancel` ending the turn with `cancelled`; verify with the tests in 2.2

## 2. Tests

- [x] 2.1 Port `tests/agents.rs` to scripts (`update`, `permission`, `stderr`+`exit`, `ignoreSigterm`) with no `AIDITOR_TEST_AGENT` left; verify `cargo test -p aiditor-services --test agents` passes and `grep -r AIDITOR_TEST_AGENT crates` finds nothing
- [x] 2.2 Add `tests/fake_agent.rs` driving the fake as an ACP client: a turn that streams, asks, writes a file and stops; prompts past the script; set mode; cancel; a bad script; verify `cargo test -p aiditor-services --test fake_agent` passes

## 3. Docs

- [x] 3.1 Record the script format in `docs/editor.md` under Testing; verify the section names every step
- [x] 3.2 Run `cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace` and the rest of CI; verify all pass
