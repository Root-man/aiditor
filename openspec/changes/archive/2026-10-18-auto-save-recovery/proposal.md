# Proposal

## Why

The editor is experimental and will crash; until now a crash loses every unsaved edit. Issue #50 adds Emacs-style auto-save files, written outside the project so repos, watchers, agents and `git status` never see them. Recovering them on launch is #51.

## What Changes

- About 2 s after the last edit, and at least every 30 s while edits keep coming, every modified file or scratch buffer whose text changed since its last auto-save is written to a recovery file.
- Recovery files live in `recovery/` under the platform data dir (`~/Library/Application Support/aiditor` on macOS, `$XDG_DATA_HOME/aiditor` on Linux), never next to the file. Each holds a JSON header line (original path, buffer name, base disk hash, time) and then the buffer's text.
- A recovery service on its own thread writes each one to a temp name and renames it, and deletes it once the buffer is saved, killed or no longer modified, or when the editor quits normally (the user already chose to discard unsaved edits).
- The file hash becomes 64-bit FNV-1a instead of std's `DefaultHasher`, so a hash stored on disk means the same thing to a later build.
- New `Msg::AutoSave`, `Effect::AutoSave` and `Effect::DeleteAutoSave`.

Not in this change: recovery on launch (#51), revert (no revert command exists yet; the "no longer modified" rule will cover it), a setting for the delays, a `--data-dir` flag.

## Capabilities

### New Capabilities
- `auto-save`: writing unsaved buffers to recovery files outside the project and removing them when they are no longer needed.

### Modified Capabilities

None.

## Impact

- `crates/aiditor-core`: `lib.rs` (timers, sweep, messages, effects), `buffer.rs` (`BufferId::number`), harness.
- `crates/aiditor-services`: new `recovery.rs`; `files.rs` hash.
- `crates/aiditor`: `main.rs` starts the service, routes the effects and drains it before quitting.
- `docs/editor.md`: Crash safety records the decisions.
- No new dependencies.
