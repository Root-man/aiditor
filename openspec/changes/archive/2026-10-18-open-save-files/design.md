# Design

## Context

`Core::open(path, text)` takes text main read with `read_to_string`; `Effect::Save` carries the rope and main writes it with `std::fs::write`, then sends `Msg::Saved(id)`. Main already holds the services channel's sender (unused) and a Tokio runtime. The harness in `aiditor-core` carries effects out itself and cannot depend on `aiditor-services`, which depends on the core.

## Goals / Non-Goals

**Goals:** the #45 rules for load and save, a hash check on every save, saves off the UI thread.

**Non-Goals:** the conflict flow (M1), file watching, other encodings (#155), `find-file` (#41: it will refuse binary files rather than open them read-only), a `read-only-mode` toggle, copying owner, group or extended attributes.

## Decisions

- **Bytes cross the boundary, format logic stays in the core.** The service reads and writes bytes; `buffer::decode` (BOM, NUL check, UTF-8, CRLF) and `encode` live in the core as pure functions, so the harness tests them without I/O. Alternative: decode in the service and send text; then the harness would duplicate it.
- **The service owns the hash.** `files::read` returns bytes and their hash; `files::write(path, bytes, expected)` re-reads the disk, compares hashes (`None` meaning no file), writes, and returns the new hash. The core only stores the `Option<u64>` and hands it back in `Effect::Save`. The hash is std's `DefaultHasher` over the bytes: change detection, not security.
- **Atomic write**: canonicalize the path (following symlinks), write `.<name>.aiditor-<pid>~` beside the target with `create_new`, copy the old permissions, `sync_all`, rename. Fall back to an in-place write when the file has `nlink > 1` or a step fails with `PermissionDenied` (the temp is removed first). Other errors fail the save rather than truncate the file in place. A path that does not canonicalize (missing file, dangling symlink) is written with `fs::write`, which creates it through the link.
- **The final newline is a buffer edit** made by `save-buffer` before encoding, like Emacs, so buffer and disk agree; point stays before it.
- **Saves are async**: main runs `files::write` in `spawn_blocking` and sends `Msg::Saved { buffer, tick, result }` on the services channel. The tick guards against edits made while the write ran: the hash is always updated, but the buffer becomes unmodified only if the tick is unchanged.
- **Unmodified saves are skipped** with Emacs's message. This also keeps a refused binary file's empty buffer from ever overwriting it.
- **Read-only** is a buffer flag checked in the raw insert and delete, so every edit path (commands, typing, undo) fails with `Buffer is read-only`.
- **Mode line**: `%%` replaces the modified flag for read-only buffers, as in Emacs; ` (DOS)` follows the name.

## Risks / Trade-offs

- [A second `C-x C-s` while the first write runs carries the old hash and is refused as changed on disk] → writes take milliseconds; queue saves if it shows up.
- [`DefaultHasher`'s algorithm is not guaranteed across Rust releases] → only compared within one run; switch to a fixed hash when #50 persists it.
- [Atomic save gives the file a new inode, dropping owner/group and xattrs] → decided in #45.
- [Encoding runs on the UI thread] → linear in file size, as the old `to_string` was.
