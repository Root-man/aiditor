# Design

## Context

Services today share one bounded channel (`aiditor_services::channel()`, 256 deep). The GUI drains it with one `cx.spawn` loop in `Editor::new`. Its `handle` dispatches each `Effect`, and `aiditor-headless`, the core's test harness and the service test support each match on `Effect` too. Grep is the model for a service the app holds state for (`Grep` in `aiditor-services`).

`portable-pty` 0.9.0 is the PTY crate the spec chose. Facts about it that shape this design, from its source:

- `CommandBuilder::new_default_prog()` runs the shell named by the builder's `SHELL` (falling back to the password database) with argv0 `-<basename>`. The builder starts from a snapshot of the process env, and the extra variables layer on top.
- A cwd that is not a directory falls back silently to `$HOME`.
- `spawn_command` calls `setsid` and makes the PTY the controlling terminal. `try_clone_reader` dups the master fd. `MasterPty::as_raw_fd` exposes the fd for `termios`.
- The parent's copy of the slave fd must be dropped after the spawn, or the master never sees EOF.
- `Child::wait` gives an `ExitStatus` with `exit_code()` and `signal()`.

## Goals / Non-Goals

**Goals:**
- Spawn, output, exit and per-terminal flow control, testable with `/bin/sh -c` scripts on macOS and Linux without GPUI.
- Message ordering per terminal by construction: started, then output, then exit.

**Non-Goals:**
- Keeping each terminal's master, writer and killer after the spawn. #215 and #216 need those and will add a `Terminals` registry, the way `Grep` holds its state.
- Anything in the core beyond accepting and ignoring the messages.

## Decisions

### Core types
`TerminalId(u64)`, chosen by the core like `BufferId`. `Effect::Terminal(TerminalOp)` with one variant for now:

```rust
TerminalOp::Spawn { id, command: Option<Vec<String>>, cwd: PathBuf, env: Vec<(String, String)>, rows: u16, cols: u16 }
```

`command: None` means the login shell. The messages:

- `Msg::TerminalSpawned { id, result: Result<(), String> }`
- `Msg::TerminalOutput { id, bytes: Vec<u8> }`
- `Msg::TerminalExited { id, code: u32, signal: Option<String> }`

They live in `lib.rs` next to the other ops. A `terminal` module waits for the emulator, which will fill it. `Core::dispatch` gets one no-op arm for the three.

`command` is not in the issue, but tests need `/bin/sh -c` scripts. The emulator and agents will need it too, for `terminal/create`.

### Service shape: `terminal::spawn(op) -> mpsc::Receiver<Msg>`
A plain function in `aiditor-services/src/terminal.rs`. It creates the terminal's bounded channel synchronously and returns the receiver at once. A spawned thread does the rest and sends everything for that terminal, including the spawn result, on that one channel. Ordering per terminal then holds by construction, and the app never sees an id before its receiver exists.

Alternative: send `TerminalSpawned` on the shared channel and hand the receiver back inside it. Rejected: the core's `Msg` would have to carry a Tokio receiver, which the core crate must not depend on.

### Channel depth 4
Four 64 KB batches cap a terminal's in-flight memory at 256 KB, on top of the kernel's PTY buffer. That is "a few batches deep" from the issue, and enough to keep the reader busy while the UI thread handles one batch.

### Threads, not the blocking pool
Each terminal runs on `std::thread`s:

1. **Spawn thread.** It sets `IUTF8`, spawns the child, drops the slave and sends `TerminalSpawned`. On success it starts the reader, then waits for the child and sends its status to the reader on a private channel.
2. **Reader thread.** It polls the PTY with a 100 ms timeout, reads into a 64 KB buffer and sends each read with `blocking_send`. It is the sole sender of output and exit messages. EOF, including Linux `EIO`, ends output; if the child has not exited yet, it waits for the real status before sending exit.

Tokio's blocking pool is for finite work, and these threads live as long as the terminal. #215 adds a third thread, the writer.

### Exit after output
The reader owns the terminal's sender. When the wait thread supplies the child's exit status, the reader keeps draining output until EOF or its 100 ms exit deadline passes. All loop work, including reading and copying ready output, consumes that grace; only the measured `blocking_send` duration extends the deadline, so a slow core cannot let exit overtake buffered output. Output and exit use the same sender, preserving their order. A background job can keep writing after the exit message on Linux; its writes do not restart the grace. macOS revokes the slave when the session leader exits, so the continued-background-output regressions run only on Linux.

Polling bounds idle reads to 100 ms so a background job holding the PTY open cannot delay the exit status. If EOF arrives before the status, the reader waits for the wait thread. Linux `EIO` is EOF (`portable-pty` already normalizes it; the service handles it explicitly too).

Alternatives rejected:
- Let the wait thread send `Exited` after a fixed wall-clock grace: a full channel lets exit overtake output on Linux.
- Drain a `FIONREAD` snapshot: Linux's readable-byte count can omit bytes still moving through the PTY buffers; the large-output slow-receiver regression catches this.

### Environment
A pure function `terminal_env(base: impl Iterator<(OsString, OsString)>, extra) -> Vec<…>` computes the variables the shell gets. It applies the editor's base env and the extra variables, then sets `TERM`, `COLORTERM`, `TERM_PROGRAM` and `TERM_PROGRAM_VERSION = env!("CARGO_PKG_VERSION")`, adds `LANG` if it is unset and removes `SHLVL`. The spawn applies the result to the `CommandBuilder` (`env_remove("SHLVL")` plus `env`). Unit tests call the function with a base that has no `LANG`, because Rust 2024 makes `remove_var` unsafe and the test process's env can't be changed safely.

The terminal's own variables are set after the extra ones and win over them, because the emulator promises xterm-256color.

### Cwd check
The spawn thread fails the spawn when `cwd` is not a directory, before calling `portable-pty`. Starting a project terminal in `$HOME` without a word would be worse than an error.

### IUTF8
`libc::tcgetattr` and `tcsetattr` on `master.as_raw_fd()`, setting `c_iflag |= IUTF8` before the spawn. `libc` defines `IUTF8` for both Apple and Linux. Add `libc` as a direct dependency; it is already in the tree through `portable-pty`. A failure is logged with `tracing::warn!` and the spawn goes on, since a terminal without `IUTF8` still works.

### App wiring
`editor.rs` handles `Effect::Terminal(TerminalOp::Spawn)` by calling `terminal::spawn` and draining the returned receiver with the same `cx.spawn` loop the shared channel uses. That loop moves into a small helper. `aiditor-headless`, `harness.rs`, `tests/support/mod.rs` and `tests/file_sync.rs` get an arm that ignores the effect, since nothing produces it yet.

## Risks / Trade-offs

- [Exit reporting uses a 100 ms deadline] → All reader-loop work consumes it except output delivery blocked by backpressure. Background output continues after exit; its writes do not restart the deadline.
- [Linux may drop the PTY's buffered output when the slave closes] → Kernels since 4.x return the remaining data before `EIO`. The "output then exit" test runs on Linux CI and would catch a regression.
- [macOS `stty -a` might not print `iutf8`] → Check on the first macOS run. If it doesn't, assert the flag with `tcgetattr` on the master instead.
- [Shells are not killed yet] → Until #216, a spawned terminal outlives its use. Nothing in the app spawns one before then, and service tests run scripts that exit.
- [Two to three threads per terminal] → Fine for the few shells per workspace the spec expects.
