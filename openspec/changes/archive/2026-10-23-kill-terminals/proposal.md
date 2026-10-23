# Proposal

## Why

Terminals can be spawned, written to and resized (#214, #215), but nothing ends them: a terminal's shell, PTY, writer thread and queued input live until the process exits, and on quit the shells are left running. Issue #216 adds Kill and kills every live terminal when the app quits.

## What Changes

- `TerminalOp::Kill { id }` sends SIGHUP, then SIGTERM, to the shell's process group and the PTY's foreground process group, and SIGKILL to both after 100 ms if the shell hasn't exited. It frees the terminal's handle (master, writer thread and queued input) at once and never blocks the caller. A Kill that lands before the shell has started kills it as soon as it does.
- Dropping the master alone doesn't hang the shell up, because the reader thread holds a duplicate of it, so Kill signals instead.
- When the app quits, every live terminal is killed before the process exits, sharing one 100 ms grace.
- The core still gets the terminal's exit message after a Kill.

Out of scope: asking before killing a live terminal (the emulator epic, since it belongs to terminal buffers); the core emitting Kill.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `terminal-processes`: adds killing a terminal and killing every terminal on quit.

## Impact

- `aiditor-core`: a new `TerminalOp::Kill` variant; the core does not emit it yet.
- `aiditor-services` `terminal`: `Terminals` tracks each terminal's shell pid, kills by id and kills all; integration tests in `tests/terminal.rs`.
- `aiditor` frontend: kills all terminals on `Effect::Quit`, before `cx.quit()`.
