# Design

## Context

`terminal::spawn(op)` runs one thread per terminal that opens the PTY, spawns the child, starts a reader thread polling the master's fd, then waits for the child. Nothing outside that thread keeps the master, so there is nowhere to send input or a size. The editor calls `spawn` for each `Effect::Terminal` and drains the returned receiver.

## Goals / Non-Goals

**Goals:** route `Write` and `Resize` to the right PTY without ever blocking the caller.

**Non-Goals:** freeing a terminal's handle and threads (Kill, #216); throttling queued input.

## Decisions

- **A `Terminals` service holds a handle per id.** `Terminals::run(op)` takes every `TerminalOp`; `Spawn` returns the output receiver for the caller to drain, as `spawn` does today, and `Write`/`Resize` return nothing. This mirrors the grep service's `run`, and keeps the core free of handles. The alternative, the core holding handles in effects, would put service objects in the core.
- **The PTY opens on the caller's thread.** `openpty` is a couple of syscalls, cheap enough for the UI thread, and it makes the master available at once, so a `Resize` or `Write` right after `Spawn` has somewhere to go. The cwd check, `fork`/`exec` and waiting stay on the spawn thread. An `openpty` failure is sent as the spawn's failure message.
- **Resize is an ioctl on the caller's thread.** `TIOCSWINSZ` never blocks, so it needs no thread, and it can't be stuck behind a blocked write. The alternative, sending resizes through the writer thread, would delay them behind a paste the program isn't reading.
- **Writes go through an unbounded channel to a writer thread per terminal.** The thread owns a duplicate of the master fd and does blocking `write_all`s in order; the caller only sends. The writer is a plain dup, not portable-pty's `take_writer`, whose `Drop` writes `\n` and EOF into the PTY.
- **The reader owns its own duplicate of the master fd** and polls it, instead of polling the master's fd number, since the master now lives in the handle on another thread.

## Risks / Trade-offs

- [Queued input is unbounded] → A paste to a program that never reads stays in memory until the terminal is killed. Pastes are small next to that memory; #216 drops the queue with the handle.
- [Handles are never removed] → A terminal's master, writer thread and queue live until #216's Kill removes them.
- [A write stuck behind a full PTY delays later input, including `\x03`] → As on any terminal, input is FIFO; a resize is not affected.
