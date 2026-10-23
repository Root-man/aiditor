# Proposal

## Why

M2's terminal buffers need a running shell to talk to. The PTY service is the first piece (#214): it starts the user's shell in a pseudo-terminal off the UI thread and streams its output to the core, without letting a flood of output (`cat` of a large file, a noisy build) stall file replies, grep or anything else the core is waiting for.

## What Changes

- A terminal service that spawns a command in a PTY of a given size, in a given directory, with extra env. The default command is the user's `$SHELL` as a login shell. Spawning runs off the UI thread, since forking takes 10-70 ms, and its success or failure comes back as a message.
- Output is read in batches of up to 64 KB and sent to the core in order, followed by the command's real exit status once it exits. Exit is taken from waiting on the child, not from the end of output, because background jobs can hold the PTY open.
- Each terminal sends on its own small bounded channel, which the app drains into the core. A core that doesn't keep up blocks only that terminal's reader, so the kernel's PTY buffer fills and the producing program blocks, while other services' replies still arrive.
- The shell starts with `TERM=xterm-256color`, `COLORTERM=truecolor`, `TERM_PROGRAM=aiditor` and `TERM_PROGRAM_VERSION`; `LANG=en_US.UTF-8` when it is unset (apps launched from the macOS Dock have none); `SHLVL` removed. The PTY has `IUTF8` set, which Linux leaves off, so canonical-mode erase removes whole UTF-8 chars.
- A cwd that is not a directory is a spawn failure, not a silent start in `$HOME`.
- The core accepts the new messages and ignores them until the emulator epic (#87).

Not in this change: writing input and resizing (#215), killing terminals and cleanup at quit (#216), the emulator and terminal buffers (#87), any command that opens a terminal.

## Capabilities

### New Capabilities
- `terminal-processes`: spawning a command or the login shell in a PTY, its environment, streaming its output and exit status, and per-terminal flow control.

### Modified Capabilities
- `core-loop`: services no longer all share one channel; each terminal has its own bounded channel, drained into the core the same way.

## Impact

- `aiditor-core`: `TerminalId`, `Effect::Terminal(TerminalOp::Spawn)`, `Msg::TerminalSpawned`, `Msg::TerminalOutput`, `Msg::TerminalExited`; dispatch ignores them. No command sends `Spawn` yet.
- `aiditor-services`: a `terminal` module. New dependency `portable-pty` (chosen in `docs/editor.md`); `libc`, already in the tree through it, for `IUTF8`.
- `aiditor` (GUI) and `aiditor-headless`, plus the core test harness and service test support: carry out or skip `Effect::Terminal`; the GUI drains each terminal's channel into the core.
- `docs/editor.md`: per-terminal channels under Output floods, the terminal env, and that bash login shells read `.bash_profile`/`.profile`, not `.bashrc`.
