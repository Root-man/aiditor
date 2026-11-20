# Proposal

## Why

Pasting a file's path into an agent prompt, a terminal or a chat is common, and today the only way is to read it off the mode line and type it. Two commands that copy the current file's path, relative to its git repository root or absolute, close that gap.

## What Changes

- New command `copy-relative-path`: copies the current buffer's file path relative to the root of the git repository holding the file (the nearest directory above it with `.git`), e.g. `src/a.rs`.
- New command `copy-absolute-path`: copies the current buffer's absolute file path, e.g. `/home/me/repo/src/a.rs`.
- Both put the path on the kill ring and the system clipboard, as `kill-ring-save` does, and echo `Copied <path>`.
- Both fail with `Buffer is not visiting a file` when the current buffer has no file. `copy-relative-path` fails with `File is not in a git repository` outside a repository.
- No key bindings; both run from `M-x`.

## Capabilities

### New Capabilities
- `file-paths`: commands that hand the current file's path to the user (copy relative or absolute).

### Modified Capabilities
None.

## Impact

- `crates/aiditor-core/src/lib.rs` and `crates/aiditor-services/src/files.rs`: `Head` gains the repository's root, which `head()` already finds.
- `crates/aiditor-core/src/builtins.rs`: two `Command` entries.
- `crates/aiditor-core/src/kill_ring.rs`: a small helper that pushes text onto the ring and emits `Effect::SetClipboard`, shared with `kill`.
- `docs/editor.md`: a line recording the commands.
- No new dependencies; no frontend change (the clipboard effect already exists).
