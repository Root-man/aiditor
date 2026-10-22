# Tasks

## 1. Native integration suite

- [x] 1.1 Add a bounded native-watcher/real-file-service/Core driver and tests for atomic external rewrites across clean workspace copies, unrelated buffers, dirty conflicts, and own-save tick plus undo history; verify `cargo test -p aiditor-services --test file_sync` with native filesystem access.
- [x] 1.2 Cover visited-file fallback for watch-disabled and zero-limit projects in the integration suite; verify both cases receive native reports and auto-revert.

## 2. Documentation and verification

- [x] 2.1 Document the suite in `docs/editor.md` and verify the existing CI matrix discovers it on macOS and Linux through `cargo test`.
- [x] 2.2 Run formatting, strict workspace clippy, workspace tests, CI structural checks and strict OpenSpec validation; verify each command succeeds before the fresh OpenSpec review.
