# Tasks

## 1. Service

- [x] 1.1 Add `TerminalGit`, `FileOp::TerminalGit` and `Msg::TerminalGit` in `aiditor-core`, and answer the op in the file service with branch, numstat line counts (none under a content filter) and upstream counts; verify with service tests against temp repositories (clean, changed, binary, ahead and behind, no upstream, not a repository, filter)

## 2. Core

- [x] 2.1 Hold the git state and a stale flag on `Terminal`, set stale on creation, a new OSC 7 directory and a `D` mark; ask each tick and drop answers for a left directory; verify with core tests
- [x] 2.2 Add terminal repositories' `HEAD` and index to the watch set, mark terminals stale on reports under their repository, and raise `files_changed` when a terminal buffer is killed; verify with a watching test
- [x] 2.3 Splice `⎇`, `+N −M` and `↑A ↓B` in front of a terminal window's right group and keep the workspace list's state column unchanged; verify with mode line and workspace list tests
- [x] 2.4 Add `ModeLine::optional` and drop those segments, last first, before the path shortens; verify with a narrow mode line test

## 3. Docs

- [x] 3.1 Describe the segments in the terminal section of `docs/editor.md`
