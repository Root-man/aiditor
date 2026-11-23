# Design

## Context

The terminal tracks its directory from OSC 7 and its commands from OSC 133 (`Terminal::cwd`, `feed_at`). A file buffer's branch comes from `FileOp::Head`, answered off the core thread by the file service, and the watch set carries each repository's `HEAD` and index so a branch switch is seen. The workspace list builds its terminal rows from `terminal_mode_line`, taking the right group's running and failed faces as its state column.

## Goals / Non-Goals

**Goals:** the three segments, refreshed without polling, cut sensibly on narrow windows.

**Non-Goals:** fetching from the remote; showing stash, conflict or untracked counts; git state in the workspace list.

## Decisions

- **One service call per ask.** `FileOp::TerminalGit { id, dir }` answers `Msg::TerminalGit { id, dir, git }` with `TerminalGit { head, branch, lines, upstream }`. The branch and `HEAD` path come from the same `repository()` the file mode line uses; lines from `git diff --numstat HEAD` (whole repository, `-` rows for binary files skipped); upstream from `git rev-list --left-right --count HEAD...@{upstream}`. `rev-list` over `git status --branch` because status walks the working tree for untracked files, and only the counts are wanted. An unborn branch fails `git diff HEAD`, so it shows no counts.
- **A stale flag on the terminal.** `Terminal` holds `git: Option<TerminalGit>` and `git_stale: bool`, set at creation, on a new OSC 7 directory and on a `D` mark. Each tick `ask_terminal_git` sends one op per stale terminal with a directory, as `ask_heads` does. An answer whose `dir` isn't the terminal's current directory is dropped, so no request counter is needed.
- **Watching.** The watch set adds each terminal repository's `HEAD` and index; `on_files_changed` marks a terminal stale when a reported path is under its repository root, and leaves those git files out of index staleness as it does for file buffers. Killing a terminal buffer raises `files_changed` so the watch set drops its files.
- **Segments are added at the window, not in `terminal_mode_line`.** The workspace list reuses `terminal_mode_line` for its state column, filtered by face; the `+N −M` counts use the running and failed faces as the diff view's counts do, so adding them there would leak into that column. The window's mode line splices them in front.
- **Giving way.** `ModeLine` gains `optional: usize`, the count of leading right segments that give way, last first, before the path shortens. Every other mode line sets 0.

## Risks / Trade-offs

- [A build writing many files under a watched root re-asks git each report] → the watcher already debounces; `git diff --numstat` is cheap on a clean index. Marked as a shortcut to coalesce if it shows.
- [Upstream counts go stale until the next fetch] → as starship's; nothing fetches by design.
