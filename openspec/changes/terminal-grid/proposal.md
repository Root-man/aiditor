# Proposal

## Why

Terminals can be spawned, written to, resized and killed (#214–#216), but their output goes nowhere: the core drops `Msg::TerminalOutput`. Issue #223 puts a VT emulator in the core so a terminal's output lands in a buffer's grid, the first step towards terminal buffers.

## What Changes

- `alacritty_terminal` joins `aiditor-core`, behind a small terminal interface (feed bytes, read the screen, collect replies), so the emulator can move off the UI thread later (docs/editor.md, Output floods).
- A buffer's content gains a second variant, the terminal grid. A terminal buffer is read-only and its text is an empty rope, so rope commands run in it harmlessly until copy mode builds text from the scrollback.
- The core maps each `TerminalId` to its buffer and feeds `Msg::TerminalOutput` into that buffer's grid. Replies the emulator writes back (cursor position and device attribute reports) go out as `TerminalOp::Write`. Title, bell and clipboard requests are ignored, as is output for an unknown terminal.
- Tests create terminal buffers through a core method; there is no command, spawn or rendering yet.

Out of scope: a command to open a terminal, spawning from the core, drawing the grid, char mode and copy mode, resize from window size, killing a terminal with its buffer.

## Capabilities

### New Capabilities

- `terminal-buffers`: buffers whose content is a terminal emulator's grid, fed by the terminal's output.

### Modified Capabilities

None.

## Impact

- `aiditor-core`: new `alacritty_terminal` dependency (no GPUI or Tokio), a `terminal` module, a `Content::Terminal` variant, a `TerminalId` → buffer map in `Core`, and `Msg::TerminalOutput` handling.
- No change to services or the frontend.
