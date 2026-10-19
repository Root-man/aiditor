# Proposal

## Why

The core's file effects (`Save`, `ListDir`, `Open`) are carried out by three hand-written executors that already disagree: the app (`Editor::handle` in `crates/aiditor/src/main.rs`), the headless binary (`send` in `crates/aiditor-services/src/bin/aiditor-headless.rs`) and the test harness (`Harness::run` in `crates/aiditor-core/src/harness.rs`). Headless errors lack the path the app's carry, the harness hashes with `DefaultHasher` while the real service uses FNV-1a, and a fix in one executor never reaches the others. The recovery thread takes the whole `Effect` enum and drops what it doesn't know with `_ => Ok(())`, so a new recovery effect could be silently ignored. Two hot-path costs ride along: `save` encodes the whole buffer on the UI thread, and `files::list` stats every directory entry.

## What Changes

- Group the core's effects: `Effect::File(FileOp)` with `FileOp::{Save, ListDir, Open}` and `Effect::Recovery(RecoveryOp)` with `RecoveryOp::{AutoSave, DeleteAutoSave, DeleteRecovery}`. `LoadSettings`, `SetClipboard` and `Quit` stay as they are.
- Add `aiditor_services::files::run(op: FileOp) -> Msg`, blocking: the one place that turns a file operation into its reply message, with one error format (`<path>: <reason>`, as the app shows today). The app runs it on Tokio's blocking pool; headless calls it directly.
- `recovery::spawn` takes `RecoveryOp`s; its `run` matches them exhaustively, with no catch-all.
- `FileOp::Save` carries the buffer's `Rope` snapshot plus `crlf` and `bom` instead of encoded bytes; `files::run` encodes on the blocking pool with a new free function `aiditor_core::buffer::encode`. `Buffer::encode` goes.
- The FNV-1a `hash` moves from `files.rs` to `aiditor_core::buffer::hash`, shared by services and the harness.
- `files::list` uses `DirEntry::file_type` and calls `fs::metadata` only for symlinks.
- A failed save is no longer logged at error level beside its echo (H4 of `m0-quick-wins`, done here if that change hasn't landed).
- Headless error texts for `Open` and `ListDir` gain the path prefix the app already shows (tooling only).

No user-visible behavior changes; the app's messages stay byte-for-byte the same.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

None. This is a refactor; `.openspec.yaml` sets `skip_specs: true`.

## Impact

- `crates/aiditor-core`: `Effect` enum and its producers in `lib.rs` (`Core::save`, `find_file`, `recover_buffers`, the `Msg::AutoSave` arm of dispatch, `Core::autosave`, `poll_completion`), `buffer.rs` (`encode`, `hash`), `harness.rs` (`Harness::run`, `open`, tests).
- `crates/aiditor-services`: `files.rs` (`run`, `list`, hash removed), `recovery.rs` (`spawn`, `run`, tests), `bin/aiditor-headless.rs`.
- `crates/aiditor/src/main.rs`: `Editor::handle`'s effect loop and the `recovery` field's type.
- No new dependencies; `aiditor-core` still has no gpui or tokio, and CI's `cargo tree` check is unchanged.
- Lands after `split-core`, which moves the effect producers; locations are given by item name as well as line.
