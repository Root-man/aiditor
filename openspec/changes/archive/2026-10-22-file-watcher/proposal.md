# Proposal

## Why

Buffers have to follow the disk: agents, other workspaces and `git checkout` change files under open buffers, and the file finder goes stale as files come and go. Issue [#80](https://github.com/Root-man/aiditor/issues/80) builds the watcher that "Decide: file watcher edge cases" (#78) designed. Auto-revert (#81), ignore patterns (#190), `watch = false` (#191) and the watch limit (#192) all build on it.

## What Changes

- A file watcher service in `aiditor-services`, on `notify`. It reports, debounced, the paths that may have changed. Event kinds are not trusted.
  - **macOS**: one recursive FSEvents watch per root. Events under ignored directories are dropped.
  - **Linux**: one non-recursive inotify watch per directory that the index's walk keeps. Ignored trees (`node_modules/`, `target/`) get no watch. A directory created later gets a watch unless ignored, and is scanned for the files already in it.
  - **Lost events** (`notify`'s rescan flag, an inotify overflow) are reported as a rescan.
- The core tells the service what to watch:
  - the roots of every project a workspace is bound to;
  - the files that file buffers visit, in any workspace.
  - A visited file that no root watch covers gets a non-recursive watch on its parent directory. That covers a file outside the roots, under an ignored directory, or in an unwatched project. The watch is shared by the buffers visiting files there and dropped with the last of them.
- Each buffer visiting a reported path, or a file under a reported directory, is checked against the disk. A rescan checks all of them.
  - A missing file puts the buffer in the deleted state: the mode line shows `[deleted]` and its disk hash becomes none.
  - A file that is there again clears the state.
  - The buffer's text, undo history and modified flag are untouched, so a clean deleted buffer doesn't stop `C-x C-c`.
  - The watcher reports the editor's own saves too; filtering happens per buffer (comparing hashes is #81).
- `C-x C-s` writes a deleted buffer's file back without asking, modified or not. A save that finds the file gone writes it too, for when the watcher missed the delete. **BREAKING** for the save refusal: a file that disappeared is no longer a reason to refuse.
- A create or delete under an indexed project's roots refreshes its file index silently, so the next `C-x p f` lists the new files and drops the gone ones.
- Defaults picked:
  - The launch `$HOME` project's root is not watched, since inotify would need a watch per directory under `$HOME`. Its open files are watched through their directories. #191 makes this the `watch = false` default.
  - Only projects bound to a live workspace are watched.
  - Debounce: paths are sent after 100 ms with no new event, and at most 1 s after the first one.
- Not included:
  - auto-revert and conflicts (#81 onwards);
  - ignore patterns (#190);
  - `watch = false` (#191);
  - the watch limit (#192);
  - live updates of an already open `C-x p f` list.

## Capabilities

### New Capabilities
- `file-watching`: what is watched (roots, uncovered visited files' directories, per platform), debounced path reports, rescans, the per-buffer check and the deleted state.

### Modified Capabilities
- `file-visiting`: `C-x C-s` writes back a deleted buffer, and a file gone from disk no longer refuses a save.
- `display-model`: the mode line shows `[deleted]`.
- `file-index`: creates and deletes under a project's roots refresh its index.

## Impact

- `aiditor-services`:
  - a `notify` dependency;
  - a new `watcher` module (service thread, debounce, per-platform watches, ignore-aware directory walk);
  - `files::run` answers `FileOp::Check`;
  - `files::write` no longer refuses a file that is gone.
- `aiditor-core`:
  - `Effect::Watch`, `Msg::FilesChanged`, `FileOp::Check` and `Msg::Checked`;
  - a `deleted` flag on `Buffer`, shown in the mode line;
  - the watch set computed after every message;
  - the save and index refresh changes;
  - the harness answers the new ops.
- `aiditor` app: starts the watcher service and routes `Effect::Watch` to it; `aiditor-headless` ignores it.
- `docs/editor.md`: a "Built in #80" note under File watching.
