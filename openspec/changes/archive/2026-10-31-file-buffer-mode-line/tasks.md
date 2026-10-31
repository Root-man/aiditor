# Tasks

## 1. Branch service

- [x] 1.1 Add `FileOp::Head(Vec<PathBuf>)` and `Msg::Heads`, with `files::head` reading `.git` and `HEAD`, and verify unit tests for a branch, a slash, a detached hash, a worktree's `.git` file and no repository
- [x] 1.2 Answer `FileOp::Head` in the harness, default no repository, and verify existing core tests see no new leftover effect

## 2. Core state

- [x] 2.1 Add `Buffer.head` and `head_asked`, sweep unasked file buffers in `handle`, store `Msg::Heads`, and verify with a harness test
- [x] 2.2 Add `HEAD` paths to `watch_set`, re-ask on a report or rescan, keep them from the index's staleness, and verify with harness tests
- [x] 2.3 Drop other paths under `.git` in the watcher and verify a real checkout through the watcher in `crates/aiditor-services/tests/file_sync.rs`

## 3. Mode line

- [x] 3.1 Add the language name to `Syntax`, `Buffer` access to it and the `mode-line-buffer-id` face, verified by tests
- [x] 3.2 Pass roots to `display::build` and build the file left group, verified by display tests for modified, read-only, both, root-relative, outside a root and major mode
- [x] 3.3 Build the file right group (region, `UTF-8 CRLF`, branch, position), moving truncation out of `wrap_lines`, verified by display tests for each segment and position
- [x] 3.4 Update existing mode line assertions, snapshots and benches, and verify `cargo test --workspace` passes

## 4. Docs

- [x] 4.1 Update `docs/editor.md`: line endings, the mode line segments, the `mode-line-buffer-id` face and the branch, and verify no `(DOS)` is left in it

## 5. Review: git status marker

- [x] 5.1 Add `changed` to `Head` from `git status` in `files::head`, verified by a test against a real repository for untracked, committed, ignored, unstaged, staged, a pattern-like name and an untouched index
- [x] 5.2 Watch `.git/index`, re-ask on it and on the file, verified by harness tests
- [x] 5.3 Show ` ±` in the branch segment, verified by a display test, and update `docs/editor.md`

## 6. Review: path and spacing

- [x] 6.1 Shorten a file's directory before its name in a narrow window, verified by a display test
- [x] 6.2 Show the root's name in a project of several roots, verified by a display test
- [x] 6.3 Reduce the separator to ` │ `, verified by frame tests and snapshots, and update `docs/editor.md`

## Workflow follow-up

- Archive after review.
