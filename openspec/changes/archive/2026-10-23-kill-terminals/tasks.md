# Tasks

## 1. Kill

- [x] 1.1 Add `TerminalOp::Kill { id }` to `aiditor-core`, and verify the workspace builds
- [x] 1.2 Share the shell's state (`Starting`, `Running(pid)`, `Killed`, `Exited`) between the handle and the spawn thread
- [x] 1.3 Kill: remove the handle, SIGHUP then SIGTERM the shell's and the foreground process groups, SIGKILL after 100 ms on a thread unless exited
- [x] 1.4 `Terminals::kill_all` with one blocking 100 ms grace, called by the editor on `Effect::Quit` before `cx.quit()`

## 2. Tests

- [x] 2.1 Test that Kill ends an interactive shell and its foreground `sleep 1000`, and the exit message arrives
- [x] 2.2 Test that Kill ends a shell trapping SIGHUP and SIGTERM
- [x] 2.3 Test that a Kill right after Spawn ends the shell
- [x] 2.4 Test that `kill_all` leaves no shell running when it returns, including a trapping one and one still starting
- [x] 2.5 Run `cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace` and the rest of CI, and verify they pass

## 3. Docs

- [x] 3.1 Record the built behavior in `docs/editor.md`'s Terminal paragraph
