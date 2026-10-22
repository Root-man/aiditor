# Design

## Context

- **The core.** It is a pure state machine. I/O goes out as `Effect`s, and answers come back as `Msg`s on one bounded Tokio channel.
  - The file service (`files::run`) answers each `FileOp` with one `Msg`, on Tokio's blocking pool.
  - Long-lived services own a thread or task: grep (`grep::Grep`), and recovery (`recovery::spawn`, a std thread fed by a std channel).
  - The harness answers effects in-process, since the core can't depend on the services crate.
- **The index walk** (`files::walk`) is `ignore::WalkBuilder` with:
  - `hidden(false)` and `require_git(false)`;
  - `.git` filtered out and symlinked directories not followed.
- **Buffers.** `Buffer::disk` is the hash as last loaded or saved, `None` when there was no file. `save` refuses an unmodified buffer, and `files::write` refuses when the disk hash differs from the expected one, a gone file included.
- **Workspaces** are bound to a project by name (`Workspace::project`). The launch `$HOME` project is the ad-hoc project whose primary is the home directory.

## Goals / Non-Goals

**Goals:**
- One service thread that owns the `notify` watcher, the watch set and the debounce.
- A core that states what it wants watched; the service works out how to watch it.
- A per-buffer check with a deleted state that #81 extends with the hash comparison.

**Non-Goals:**
- auto-revert, conflicts;
- ignore patterns, `watch = false`, the limit;
- typed events;
- live updates of an open finder.

## Decisions

- **`Effect::Watch(WatchSet { roots, files })`: the core sends the whole desired set**, not deltas.
  - The core computes it at the end of `Core::handle`, like the autosave and preview hooks, and sends it only when it differs from the last one sent.
  - `roots`: the roots of every project bound to a live workspace, minus the `$HOME` project's.
  - `files`: every file buffer's path, sorted and deduplicated.
  - One hook catches every change: a visit, a kill, a workspace made or killed, a project edited or reloaded. No call site has to remember to update it.
- **The service (`watcher::Watcher::spawn(send)`, then `Watcher::set(WatchSet)`)** runs one std thread.
  - The `notify` handler and `set` both feed one channel the loop reads, so the loop is the only owner of state.
  - `send` is a closure (`blocking_send` on the services channel in the app, a std channel in tests).
  - It holds, per root, the set of kept directories, from a `WalkBuilder` walk with the index's filter that keeps only directories.
  - **Desired watches.**
    - macOS: each root recursive.
    - Linux: each kept directory non-recursive.
    - Both: the parent directory of each visited file whose parent is not a kept directory, non-recursive.
  - The service diffs the desired watches against the current map and calls `watch` and `unwatch` for the difference. Errors are logged at info, since a root can vanish.
- **Path filter.** A raw event path is kept when it is a kept directory, or its parent is a kept directory or an extra watched directory. This drops events under ignored directories on macOS, which watches the whole root recursively.
  - On Linux the filter only drops what a watch on another root's directory would report anyway.
  - An existing file the index would ignore, directly inside a kept directory (`*.log` in `src/`, `.DS_Store`), is dropped too, unless a buffer visits it. Once per report, each such parent is listed one level deep with the index's walk to see which entries it keeps. A gone path is never dropped, since a gone file can't be listed. This stops writes to build outputs from re-walking the index.
- **FSEvents reports canonical paths** (`/private/var/...` for `/var/...`). Each watched path is canonicalised when watched. An event path under a canonical form is rewritten to the path as the core knows it.
- **Debounce in the loop.** Raw paths go into a `BTreeSet`. With paths pending, the loop waits with `recv_timeout` until 100 ms after the last event, or 1 s after the first. It then flushes:
  1. **New directories.** A reported path that is a directory, whose parent is kept but which is not kept itself, is checked against the ignore rules by walking its parent one level deep (`max_depth(1)`, so ancestor ignore files apply). When kept, it is walked fully: its kept subdirectories join the root's set and its files are added to the report. The watches are then updated, so on Linux the new directories get inotify watches.
  2. **Gone directories.** A kept directory that no longer exists leaves the set, with its descendants.
  3. **Existence.** Each path is stat'ed and `Msg::FilesChanged { paths: Vec<(PathBuf, Disk)>, rescan }` is sent, with `Disk::{Gone, File, Dir}` saying what the path is now. Telling a file from a directory lets the index ignore directory events.
  4. **Lost events.** A `notify` event with the rescan flag, or a `notify` error, sets `rescan`. Linux's queue overflow comes through as that flag. A flush with nothing but `rescan` still sends.
- **The per-buffer check is a file op: `FileOp::Check(Vec<PathBuf>)` → `Msg::Checked(Vec<(PathBuf, Option<u64>)>)`.**
  - The service reads and hashes each path, leaving out ones it can't read (logged).
  - On `FilesChanged`, the core collects the paths of file buffers in every workspace that equal or lie under a reported path (`starts_with`), or all of them on a rescan, and sends one `Check`.
  - On `Checked`, every buffer visiting a path with `None` gets `deleted = true, disk = None`; one with `Some` gets `deleted = false`.
  - The hash is not compared yet; #81 adds that.
  - Existence alone (from `FilesChanged`) is not enough: a reported directory, or a rescan, says nothing about the files under it, and #81 needs the hash anyway.
- **`Buffer::deleted: bool`, not "`disk` is `None`".** A buffer visiting a file that never existed also has no hash, and must not show `[deleted]`. The mode line shows ` [deleted]` after ` (DOS)`.
- **Saving.**
  - `save` skips `(No changes need to be saved)` for a deleted buffer.
  - `on_saved` clears `deleted` on success.
  - `files::write` (and the harness's fake save) refuses only when a file is there with a different hash. A gone file is written back, as the decision says, even when the watcher missed the delete.
  - `projects::write` shares `files::write`, so a project file deleted under the editor is rewritten on the next project edit; that is acceptable for the same reason.
- **Index refresh.** On `FilesChanged`, for each project with an index, the core starts a quiet walk when either:
  - some reported path under one of its roots is a file that is not in the index and was not reported before (`FileIndex::missed`, so writes to an ignored visited file such as `.env` start one walk, not one each); or
  - it is gone while the index holds it or files under it; or
  - the report is a rescan.
  - A quiet walk is the same `FileOp::Index`. `FileIndex::loud` is set by a walk the user asked for and kept while it is pending, even if a quiet walk replaces it. `indexed()` skips the echo and the open finder's `add_candidates` (which would duplicate the list) unless the answer is loud.
  - shortcut: a create or delete re-walks the whole project; update the index per path if large projects lag.
- **Debounce numbers.** 100 ms quiet, 1 s cap. `git checkout` of many files lands in one or a few reports, and an editor's atomic save (delete plus create) reads as one change.
- **The app** spawns the watcher in `Editor::new` with a closure that `blocking_send`s on the services channel. `aiditor-headless` ignores `Effect::Watch`: its key scripts don't need it.

## Risks / Trade-offs

- [A large root on Linux takes one inotify watch per directory, and the walk to find them runs on the watcher thread] → reports wait during the walk. The walk is the index's, which is fast. #192 adds the limit.
- [`$HOME` isn't watched until #191] → files opened there are still watched through their directories.
- [A new directory's ignore check reads ancestor ignore files above the root] → the index walk does the same, so the two agree.
- [Events can arrive between the walk and the watch on a new directory] → files already in the directory are reported by the scan.
- [A check can race a save: a check that read the file before the save wrote it back can answer after the save, marking the buffer deleted again] → the next watcher report of that write clears the state, and the save after that is refused until #81 reverts the buffer. That is safe, and rare.
- [A kept directory deleted and recreated within one report is neither new nor gone, so on Linux its inotify watch, dropped by the kernel with the old directory, is not re-added] → rare; a later rescan or a parent's event doesn't repair it either, so it waits for the root to be re-watched. Accepted for now.
- [Tests depend on real OS notification timing] → service tests wait up to a few seconds for a report and assert on what arrives, not on exact timing beyond "one report".
