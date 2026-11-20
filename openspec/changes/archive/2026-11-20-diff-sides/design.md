# Design

## Context

`files.rs` already runs `git status` for #500 with safety flags and a repository-filter check, and `buffer::diff` diffs texts by line with `similar` for revert. The diff view will hold a file's old side and redraw the new side's buffer after each edit, so it needs both a git read and a pure re-diff off the core thread.

## Goals / Non-Goals

**Goals:** read a file on any side, answer its texts and changed line ranges; re-diff a held text against a buffer snapshot; list files that differ between two sides; read snapshot trees from another object directory.

**Non-Goals:** staging, discarding, taking snapshots (the agent turn task), the index as a listing side (status covers it), word-level diffs, untracked files in a tree-ish → working tree listing (the turn snapshot adds them to its tree).

## Decisions

- **Sides.** `GitSide::{Tree(String), Index, WorkingTree}`. A tree or index side is read with `git cat-file blob <tree-ish>:<path>` (`:0:<path>` for the index), which runs no filter; the working tree is read from disk with `read()`, so its size limit holds. A failed `cat-file` is a missing path when `git rev-parse --verify --quiet <tree-ish>^{tree}` succeeds (always for the index), otherwise git's error.
- **Texts** go through `buffer::decode`, so they match what a buffer visiting the file holds, and its reason ("binary file", "not valid UTF-8") becomes the answer's error, prefixed with the path. Texts are `Rope`s so the core keeps the old side and sends it with each re-diff cheaply.
- **Ranges.** `buffer::hunks(old, new)` merges `similar`'s adjacent non-equal line ops into `Hunk { old, new }` line ranges, one list both the inline and a later side-by-side view draw from.
- **Re-diff** is its own op, `FileOp::DiffLines { request, old, new }`, answering only ranges; the texts are already the core's.
- **Listing** runs `git diff --name-status -z --no-ext-diff --no-relative <old> [<new>] -- .` in the root, so paths are relative to the repository's top like status's. The status letter parser is shared with `porcelain_v2`.
- **Objects.** `GitObjects { directory, alternates }` sets `GIT_OBJECT_DIRECTORY` and `GIT_ALTERNATE_OBJECT_DIRECTORIES` on the git commands.
- **Safety.** A shared `git(dir)` builds the command with `--no-optional-locks`, `--literal-pathspecs` and `core.fsmonitor=false`; status and `changed()` use it too. A tree-ish starting with `-` is refused before git runs, since `git diff --output=…` writes a file. Listing against the working tree checks `own_filter` as status does; tree reads and tree-to-tree listings run no filter.
- **Request ids.** Each op carries a `request: u64` the core matches to the view's current file.

## Risks / Trade-offs

- An unmerged file has no stage-0 index entry, so its index side reads as empty. The status mode lists it as unmerged; the view can say so later.
- `cat-file` reads a blob whole, with no size limit; shortcut, check `cat-file -s` first if a huge blob turns up.
