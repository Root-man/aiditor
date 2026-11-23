# Proposal

## Why

A file buffer's mode line names its branch, but a terminal's doesn't, though a terminal is where git is run. Issue #551 asks a terminal's mode line to show the shell directory's branch, the lines changed and how the branch stands against its upstream, as a shell prompt would.

## What Changes

- A terminal's mode line shows, at the start of its right group, `⎇ <branch>`, `+N −M` (lines changed against `HEAD`, staged and unstaged together) and `↑A ↓B` (commits ahead of and behind the upstream) for the repository holding the shell's directory.
- The file service answers a directory's repository state: branch, line counts and upstream counts.
- The state is asked again when the shell's directory changes, when a command ends, and when the watcher reports a change in the repository, its `HEAD` or index included.
- On a narrow window these segments give way, the upstream first, before the directory is shortened.
- The workspace list's state column is unchanged: it still shows only the running command and the exit status.

## Capabilities

### New Capabilities

### Modified Capabilities
- `terminal-buffers`: the terminal mode line gains the git segments, when they are asked again, and how they give way on a narrow window.

## Impact

- `aiditor-core`: a new `FileOp`/`Msg` pair, the terminal's git state, the mode line, the watch set and the narrow-bar fitting (`display.rs`, `terminal.rs`, `watching.rs`, `lib.rs`, `harness.rs`).
- `aiditor-services`: the file service runs `git diff --numstat HEAD` and `git rev-list --left-right --count HEAD...@{upstream}` in the directory.
- `docs/editor.md`: the terminal section describes the segments.
