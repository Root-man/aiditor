# Proposal

## Why

Terminals can be spawned and stream their output (#214), but nothing can type into them or tell them their size, so no shell is usable yet. Issue #215 adds input and resize, with input that never blocks the UI thread, even when a large paste goes to a program that isn't reading.

## What Changes

- `TerminalOp::Write { id, bytes }` sends bytes to a terminal's PTY, in order, through a writer thread of its own, so the caller returns at once even when the PTY's input buffer is full.
- `TerminalOp::Resize { id, rows, cols }` sets the PTY's window size, which the kernel reports to the foreground job with `SIGWINCH`.
- The terminal service keeps a handle per spawned terminal so these ops reach it; the editor routes them there. Ops for an unknown terminal, or one whose process has gone, are ignored.
- Canonical-mode Backspace over a multibyte char is now checked end to end, through PTY input (moved here from #214).

Out of scope: killing terminals and freeing their handles (#216), terminal buffers and key encoding (the emulator epic).

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `terminal-processes`: adds writing input and resizing; the UTF-8 line editing requirement gains a Backspace scenario.

## Impact

- `aiditor-core`: two new `TerminalOp` variants; the core does not emit them yet.
- `aiditor-services` `terminal`: a `Terminals` service that spawns terminals and holds their handles; integration tests in `tests/terminal.rs`.
- `aiditor` frontend: holds the `Terminals` service and passes every `TerminalOp` to it.
