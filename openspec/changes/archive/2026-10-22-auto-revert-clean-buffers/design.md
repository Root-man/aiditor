# Design

## Context

- The watcher reports paths as `Msg::FilesChanged`. The core sends one `FileOp::Check` for the visited files at or under them, and the file service answers `Msg::Checked(Vec<(PathBuf, Option<u64>)>)`, each file's hash or `None` when gone.
- `Core::on_checked` sets `deleted` and clears `disk` for a gone file; it leaves a present file's hash unused.
- `FileOp::Revert { buffer, tick, path, text }` reads the file, diffs `text` against it off the core thread, and `on_reverted` applies the diff as one undo group, dropping it when the buffer's tick moved. It leaves the buffer unmodified with the file's hash, line ending and BOM.
- `Buffer::disk` is the hash as last loaded, saved or reverted. `on_saved` sets it before the watcher can report the write, since the report waits at least 100 ms.

## Goals / Non-Goals

**Goals:**
- Clean buffers follow their file in every workspace; the saving buffer is left alone.

**Non-Goals:**
- conflicts for dirty buffers (#83);
- echoing auto-reverts.

## Decisions

- **`on_checked` moves to `Ctx`.** For each buffer visiting a checked path:
  - `None`: deleted, `disk = None`, as before.
  - `Some(hash)`: not deleted; when `disk != Some(hash)` and the buffer is unmodified, push `FileOp::Revert` with its id, tick, path and text.
  - Otherwise nothing: an equal hash is the buffer's own save, and a modified buffer keeps its hash so a save is still refused.
- **The revert re-reads the file.** The check's hash only decides whether to revert. A check that read the file before a save landed, and answers after `on_saved`, starts a revert whose diff against the saved text is empty: harmless, it just sets the same hash again. The same goes for the save-versus-check race #80 noted, where a check marks a just-saved buffer deleted: the next report reverts it to no change and restores its hash.
- **Edits during the revert** move the tick, so `on_reverted` drops the diff; the buffer is then modified with the old hash, which is #83's conflict.
- **`Buffers::iter_mut` yields `(BufferId, &mut Buffer)`**, its only caller being `on_checked`, which needs the id for the revert.
- **Silent.** No echo for an auto-revert. Errors still echo through `on_reverted`, which doesn't know who asked; a flag to tell auto from manual reverts isn't worth the plumbing yet.

## Risks / Trade-offs

- [A file turning binary under an open clean buffer echoes `<path>: binary file` on each change] → rare, and it says why the buffer stopped following the file.
- [A file deleted between the check and the revert's read echoes `File <path> no longer exists`] → the next report marks the buffer deleted; rare.
