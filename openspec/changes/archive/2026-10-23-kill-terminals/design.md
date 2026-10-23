# Design

## Context

`Terminals` holds a `Handle` per id: the PTY master and the writer thread's sender. The shell is spawned and waited for on the terminal's own thread, so the `Child` (and its pid) never reaches the handle. portable-pty starts the child with `setsid`, so the shell leads its own session and process group, with the PTY as its controlling terminal. The reader thread owns a duplicate of the master fd, so dropping the handle's master doesn't close the PTY and the shell gets no hangup.

## Goals / Non-Goals

**Goals:** Kill ends the shell and its foreground job, even one trapping SIGHUP and SIGTERM, without blocking the UI thread; quitting leaves no shells behind.

**Non-Goals:** confirming before a kill; reaping background jobs a job-control shell left in other process groups (as a real terminal, they get the kernel's hangup and nothing more).

## Decisions

- **The handle shares the shell's state with the spawn thread**, as `Arc<Mutex<Shell>>` with `Starting`, `Running(pid)`, `Killed` and `Exited`. The spawn thread sets `Running(pid)` after spawning, or kills at once if it finds `Killed`; it sets `Exited` after `wait` returns, so Kill doesn't signal a reaped shell's pid. Kill reads it under the lock. The alternative, spawning on the caller's thread, would put `fork`/`exec` back on the UI thread.
- **Signal the shell's process group and the PTY's foreground process group.** The shell's pid is its process group id (`setsid`); `tcgetpgrp` on the master gives the foreground job's group, which a job-control shell puts in a group of its own. A non-interactive shell's children share its group.
- **SIGHUP, SIGTERM, then SIGKILL after 100 ms** on a thread per Kill, sent to both groups even if the shell has exited by then, so a foreground job ignoring SIGHUP and SIGTERM doesn't outlive a shell that died of SIGHUP. On quit, `kill_all` signals every terminal, sleeps 100 ms once on the UI thread, then SIGKILLs those still running: a quit may block for 100 ms, a Kill never does.
- **Kill removes the handle at once.** Dropping the sender ends the writer thread; the reader keeps going until EOF and still sends the exit message, so the core learns the terminal ended the same way as any exit.

## Risks / Trade-offs

- [pid reuse between the shell's reaping and the SIGKILL] → A group id can't be reused while the group has members; an emptied one would have to be reused, as a new group's leader, within 100 ms.
- [A spawn slower than the grace on quit] → Its thread may not kill the shell before the process exits; marked as a shortcut in `kill_all`.
- [The foreground group is read once, at Kill] → A job that starts in the 100 ms grace in a new group is missed; the shell's death hangs it up anyway.
- [Quit blocks 100 ms when terminals are live] → Short, and only on quit.
