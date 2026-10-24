# Proposal

## Why

The core holds a terminal grid (#223) and the service runs PTYs (#215, #216), but nothing opens a terminal buffer, sizes it to its windows, or ties the shell's life to the buffer's. Issue #224 adds that, so a terminal becomes an ordinary buffer of a workspace.

## What Changes

- `M-x vterm` spawns the login shell in a new terminal buffer `vterm<n><workspace>` of the current workspace and shows it in the selected window; the shell starts in the current file's directory, else the project's primary root.
- The grid and PTY follow the smallest window of the current workspace showing the terminal (Emacs's `window-adjust-process-window-size-smallest`), resized after any message that changes it.
- The shell exiting kills its buffer; a spawn failure kills the buffer and echoes the reason.
- Killing a terminal buffer by any path (`kill-buffer`, a workspace kill) kills its shell.
- `kill-buffer` on a live terminal asks `Buffer <name> has a running process; kill it? `; quitting asks `Active processes exist; kill them and exit anyway? ` after the modified-buffer questions.

Not in scope: drawing the grid (#225), typing into it (char mode), a `rename-buffer` command (none exists yet; terminal buffers are ordinary buffers for it).

## Capabilities

### New Capabilities

### Modified Capabilities
- `terminal-buffers`: opening terminal buffers with `vterm`, sizing them to their windows, and tying the shell's life to the buffer.
- `quitting`: quitting asks before killing live terminals.

## Impact

- `aiditor-core`: `vterm` command, terminal messages (`TerminalSpawned`, `TerminalExited`), a post-message pass in `Core::handle` that kills orphaned terminals and resizes grids, `kill-buffer` and `save-buffers-kill-terminal` queries, `Terminal::resize`.
- `aiditor` frontend: none; it already runs `TerminalOp`s and kills every terminal on quit.
- `docs/editor.md`: record what was built.
