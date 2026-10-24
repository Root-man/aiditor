# Proposal

## Why

A terminal buffer visits no file, so `C-x C-f` from it starts in the editor's working directory and `vterm` from it starts in the project root, not where the shell is. The terminal already tracks OSC 7 ([#243](https://github.com/Root-man/aiditor/pull/243)); this uses it (issue #244).

## What Changes

- A terminal's directory starts as the directory its shell was spawned in; OSC 7 then replaces it.
- A terminal buffer's directory is its terminal's directory, so `find-file` (and every other command taking the current buffer's directory) starts there.
- `vterm` from a terminal buffer spawns its shell in that terminal's directory.

## Capabilities

### New Capabilities

### Modified Capabilities
- `terminal-buffers`: a terminal starts with its spawn directory; `vterm` from a terminal buffer starts in its directory.
- `file-visiting`: `find-file`'s starting directory is a terminal buffer's directory.

## Impact

`aiditor-core`: `terminal.rs` (`vterm`, `Terminal`'s directory), `visiting.rs` (`Core::directory`). No API or dependency changes.
