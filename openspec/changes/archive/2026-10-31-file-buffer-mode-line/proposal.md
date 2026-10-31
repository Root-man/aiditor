# Proposal

## Why

A file buffer's mode line still shows Emacs's `--`/`**`/`%%` and the bare buffer name. Frames 1 and 9 of the visual direction give it the segments a file needs: where the file is, its mode, its branch and where point sits (#356).

## What Changes

- A buffer visiting a file gets its own left group: `●` when modified, `⊘` when read-only, the directory relative to its project root, the file name in bold, then `[deleted]`/`[conflict]`, then the major mode.
- Its right group adds the active region's size, `UTF-8 CRLF` (replacing `(DOS)`), the VCS branch and the position (`Top`, `Bot`, `All`, `n%`) after `line:col`.
- The file service finds a file's repository and branch from `.git`, and the watcher follows that repository's `HEAD`, so a checkout outside the editor updates the mode line.
- The branch segment gains ` ±` when git reports the file changed or untracked (added in review), so the file service runs `git status`, and `.git/index` is watched as well.
- Other buffers keep the mode line they have; each gets its own task.

Skipped: the language version frame 9 draws ("when the project pins one"), which the issue leaves out, and any encoding but UTF-8 (#155).

## Capabilities

### New Capabilities

- `vcs-branch`: finding the branch of a visited file's repository and following it as it changes.

### Modified Capabilities

- `display-model`: a file buffer's mode line segments and their faces.

## Impact

`aiditor-core`: `display.rs`, `frame.rs`, `syntax.rs`, `buffer.rs`, `watching.rs`, `lib.rs`, `settings.rs`, `settings.toml`, the harness, and about 30 mode line assertions and the snapshots. `aiditor-services`: `files.rs`. `docs/editor.md`. No new crate; the marker needs a `git` executable and is absent without one.
