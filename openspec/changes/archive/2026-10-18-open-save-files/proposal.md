# Proposal

## Why

The walking skeleton writes files with a plain `std::fs::write` on the UI thread: a save can overwrite a change made on disk by another program, truncates the file before writing, and reads only LF UTF-8 text. Issue #46 moves file reads and writes into a service and applies the file-handling rules decided in #45, so a save never silently clobbers or mangles a file.

## What Changes

- File reads and writes move into a file service in `aiditor-services`; saves run off the UI thread and report back to the core.
- Loading records the hash of the file's bytes. `C-x C-s` first compares the disk's hash with it and refuses with a message when the file changed on disk.
- Loading strips a UTF-8 BOM and turns CRLF into LF when the first line break is CRLF; saving writes both back. The mode line shows `(DOS)` for CRLF files.
- A save adds a missing final newline to the buffer, as Emacs's `require-final-newline`.
- A file with a NUL byte in its first 8 KB, or not valid UTF-8, is refused: named on the command line it opens as an empty read-only buffer with the reason in the echo area, instead of the app exiting. The mode line shows `%%` for a read-only buffer, and edits to it fail with `Buffer is read-only`.
- Saves are atomic (temp file in the same directory, mode copied, renamed over the target); symlinks are followed and the link kept; a file with several hard links, or in a directory that refuses the temp file, is written in place.
- `C-x C-s` in an unmodified buffer writes nothing and says `(No changes need to be saved)`, as in Emacs. A finished save says `Wrote <path>`, a failed one shows its error in the echo area instead of only logging it.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `file-visiting`: loading rules (BOM, line endings, binary refusal), the disk hash check, final newline, atomic saves keeping mode and links, save messages.
- `display-model`: the mode line shows `%%` for read-only buffers and `(DOS)` for CRLF files.

## Impact

- `crates/aiditor-services`: new `files` module (read, write with hash check), its tests; `tempfile` as a dev-dependency (already in the lock).
- `crates/aiditor-core`: `buffer.rs` (decode/encode, line-ending, BOM, read-only and disk-hash fields), `lib.rs` (`Core::open` takes bytes, `Effect::Save` and `Msg::Saved` carry bytes, hash and result), `display.rs` (mode line), harness and its tests.
- `crates/aiditor`: `main.rs` reads the startup file through the service and runs saves on Tokio's blocking pool, sending `Msg::Saved` on the existing channel.
- `docs/editor.md`: the defaults chosen here, under File handling.
