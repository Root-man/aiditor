# Proposal

## Why

The Core harness synthesizes filesystem replies and watcher messages, while native watcher tests stop at event delivery. Issue #84 needs coverage of their assembled behavior so an external write reliably reaches the right buffers on macOS and Linux.

## What Changes

- Add services integration tests using temporary Git roots, the native watcher, the real file service, and Core's normal message/effect loop.
- Cover clean copies across workspaces, unrelated buffers, dirty conflicts, own-save suppression with undo history, atomic external saves, and visited-file fallback for disabled or size-limited root watching.
- Document the suite and retain the existing two-platform CI test command.

## Capabilities

### New Capabilities

None. This change only tests existing behavior; `skip_specs: true` applies.

### Modified Capabilities

None. Existing file-watching, file-reverting and file-conflicts requirements remain unchanged.

## Impact

`crates/aiditor-services/tests/file_sync.rs` and `docs/editor.md`. No production API, dependency, configuration default or CI workflow changes.
