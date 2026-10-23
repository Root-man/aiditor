# Tasks

## 1. Core types

- [x] 1.1 Add `TerminalId`, `TerminalOp::Spawn`, `Effect::Terminal` and `Msg::TerminalSpawned` / `TerminalOutput` / `TerminalExited` to `aiditor-core/src/lib.rs`, with a no-op dispatch arm. Add an ignoring arm to `harness.rs`, `aiditor-headless.rs`, `tests/support/mod.rs` and `tests/file_sync.rs`, and a placeholder arm to `editor.rs` that 3.1 fills in. Verify `cargo build --workspace --all-targets` passes and the core's existing tests still pass.

## 2. The terminal service

- [x] 2.1 Add `portable-pty = "0.9.0"` and `libc` to `aiditor-services`. Write `terminal_env(base, extra)` as the design describes. Verify with unit tests for the terminal variables, the `LANG` default with a base that has none, an existing `LANG` kept, `SHLVL` removed and extra variables applied.
- [x] 2.2 Add `terminal::spawn(op) -> Receiver<Msg>`, following the design:
  - a channel 4 deep;
  - a spawn thread that checks cwd, sets `IUTF8`, spawns, drops the slave and sends `TerminalSpawned`;
  - a reader thread sending batches of up to 64 KB;
  - a wait thread that supplies the child status to the reader; the reader sends `TerminalExited` after EOF or 100 ms spent polling for more input, excluding output backpressure;

  Verify with `/bin/sh -c` tests in the services crate:
  - `printf a; printf b; exit 3` gives `Spawned`, output `ab`, then `Exited` with code 3;
  - a missing program and a missing cwd each give a failure and nothing more;
  - 1 MB of output arrives whole, in batches of at most 64 KB;
  - `sleep 5 & exit 0` reports `Exited` within about a second, as does a continuously writing background job;
  - `stty -a` lists `iutf8` without a leading `-`;
  - `env` shows the terminal variables, and an extra variable reaches the process.
- [x] 2.3 Test the login shell. With no command, `SHELL=/bin/bash` and `HOME` set to a temp dir whose `.bash_profile` runs `echo "$0"; exit`, the output is `-bash`. Do the same with zsh through `ZDOTDIR` and `.zprofile`, skipping when zsh isn't installed. Verify both pass on macOS.
- [x] 2.4 Test flow control. A terminal runs `head -c 8000000 /dev/zero; touch done` and its receiver is held but not drained. Then:
  - after 500 ms `done` doesn't exist, and a `files::run` reply sent on the shared channel is received;
  - draining then yields all 8,000,000 bytes, and `done` appears.

  Verify that the test passes.

## 3. Wiring and docs

- [x] 3.1 In `editor.rs`, move the shared channel's drain loop into a helper. Use it both for that channel and for each receiver `terminal::spawn` returns on `Effect::Terminal`. Verify the GPUI tests in `aiditor` still pass.
- [x] 3.2 Update `docs/editor.md`:
  - under Output floods, each terminal has its own bounded channel, 4 batches deep, so a flood stalls only its own reader;
  - under Subsystems (Terminal), the terminal's env and `IUTF8`;
  - under Process environment, bash login shells read `.bash_profile`/`.profile` and not `.bashrc`.

  Verify the docs match the `terminal-processes` spec.

## 4. Checks

- [x] 4.1 Verify that `cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace` and the rest of `.github/workflows/ci.yml` pass. The terminal tests must pass on macOS here and on Linux in CI.
  Verification: [GitHub Actions run 39961592738](https://github.com/Root-man/aiditor/actions/runs/39961592738) passed on Linux and macOS for implementation commit `c10af6e`, including formatting, clippy, architecture checks and both complete workspace test suites. The existing file-size-limit test was excluded only from the local macOS run with user approval; CI ran it unchanged.

- [x] 4.2 Run `openspec validate spawn-shell-in-pty --strict` and verify it passes.
