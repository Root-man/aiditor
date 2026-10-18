# Tasks

## 1. File service

- [x] 1.1 Add `aiditor-services::files` with `read` and `write` (hash check, atomic write keeping mode, symlink, hard-link and permission fallbacks); verify with tests for a changed file, a new file, mode 0754, a symlink, a hard link and a non-writable directory

## 2. Core

- [x] 2.1 Add `buffer::decode`/`encode` and the buffer's `crlf`, `bom`, `disk` and `read_only` fields, read-only failing raw edits; verify with unit tests for BOM, CRLF, LF, NUL and invalid UTF-8
- [x] 2.2 `Core::open` takes bytes and hash, refusing binary files into an empty read-only buffer with a message; `save-buffer` skips unmodified buffers, adds the final newline and emits bytes and hash; `Msg::Saved` carries tick and result; verify with harness tests for every file-visiting scenario
- [x] 2.3 Mode line shows `%%` and ` (DOS)`; verify with display tests

## 3. App

- [x] 3.1 `main.rs` loads through `files::read` and saves via `spawn_blocking` on the services channel; verify the GPUI tests still pass
- [x] 3.2 Record the defaults in `docs/editor.md` under File handling; verify the text names them

## 4. Checks

- [x] 4.1 `cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D warnings` and `cargo test --workspace` pass

## Workflow follow-up

- Archive the change before pushing the PR.
