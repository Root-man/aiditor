# Proposal

## Why

A terminal's mode line still shows `%%`, the bare name and `[char]`. Frames 5 and 9 of the visual direction give it what a terminal needs at a glance: where the shell is, what is running and whether the last command failed (#358).

## What Changes

- A terminal buffer gets its own left group: the buffer name in `mode-line-buffer-id`, then the shell's directory in `mode-line-dim`, `~` for the home directory. `%%` goes.
- Its right group gains `◉ <command>` in `status-running` while a command runs and `exit <n>` in `status-failed` after a non-zero exit, before the mode, which is now `char` or `copy` without brackets, then `line:col` in copy mode.
- The terminal keeps the running command and the last failure from OSC 133 marks. They survive a full scrollback, where the marks themselves stop.
- zsh's integration sends the command line with its `C` mark, as `cmdline_url=<percent-encoded>`, the form kitty and Ghostty use.

Skipped: bash and fish integration (zsh is the only one there is), and showing a command for a shell whose `C` mark has no `cmdline_url`; that shell still gets the exit status.

## Capabilities

### New Capabilities

### Modified Capabilities

- `display-model`: a terminal buffer's mode line segments and their faces.
- `terminal-buffers`: the mode line requirement, and following the running command and last exit.
- `terminal-processes`: zsh's `C` mark carries the command line.

## Impact

`aiditor-core`: `display.rs`, `terminal.rs`, and the frame tests' mode line assertions. `aiditor-services`: `zshenv` and `tests/terminal.rs`. `docs/editor.md`. No new crate.
