# Design

## Context

See proposal.md for the motivation. Locations below are given as item names, with line numbers as of commit `8fa1ddd`. `split-core` will move most of `crates/aiditor-core/src/lib.rs` into modules before this lands, so go by the names.

- `Effect` (`lib.rs:191-224`) is flat: `Save { buffer, tick, path, bytes, hash }`, `ListDir(PathBuf)`, `Open(PathBuf)`, `LoadSettings`, `SetClipboard(String)`, `Quit`, `AutoSave { buffer, name, path, hash, text: Rope }`, `DeleteAutoSave(BufferId)`, `DeleteRecovery(PathBuf)`.
- Producers: `Msg::AutoSave` arm of `Ctx::dispatch` (`lib.rs:575`), `Ctx::find_file` (`lib.rs:1256`), `Ctx::recover_buffers` (`lib.rs:1342`), `Ctx::save` (`lib.rs:1373`, encodes with `buffer.encode()` at `:1377`), `Core::autosave` (`lib.rs:1968-1978`, splices `DeleteAutoSave` in before `Quit`), `Core::poll_completion` (`lib.rs:2015`). `Quit`, `SetClipboard` and `LoadSettings` producers don't change.
- Consumers: `Editor::handle` (`crates/aiditor/src/main.rs:121-183`), `send` in `crates/aiditor-services/src/bin/aiditor-headless.rs:51-86` (plus its `Service` alias, `:18`), `Harness::run` (`crates/aiditor-core/src/harness.rs:110-185`), `recovery::run` (`crates/aiditor-services/src/recovery.rs:30-83`, catch-all at `:82`), and tests: `harness.rs:2784` (`edits_during_a_save_keep_the_recovery_file`), `recovery.rs:139-212`.
- The reply messages `Msg::Saved`, `Msg::Listed`, `Msg::Opened` (`lib.rs:68-85`) keep their shapes.
- The app formats `ListDir`/`Open` errors as `"<path>: <err>"`; `files::write` already formats its own (`"<path>: <err>"`, `"<path> changed on disk; not saved"`). Headless uses bare `e.to_string()`. The specs (`file-visiting`) only require that the error is shown and names the file; they quote no reason text.

## Goals / Non-Goals

**Goals:**
- One function maps file operations to reply messages; the app and headless both call it.
- The type system makes the recovery thread handle every recovery effect.
- The UI thread no longer encodes a buffer to save it.
- The harness and the services hash with the same function.

**Non-Goals:**
- Running the harness through the real `files::write` (atomic save, symlinks, hard links). `files.rs`'s own tests and `crash_recovery.rs` cover that path.
- Moving the recovery thread onto Tokio, or touching clipboard and settings effects.

## Decisions

### 1. Grouped effects: `Effect::File(FileOp)` and `Effect::Recovery(RecoveryOp)`

```rust
pub enum Effect {
    File(FileOp),
    Recovery(RecoveryOp),
    LoadSettings,
    SetClipboard(String),
    Quit,
}
pub enum FileOp {
    Save { buffer: BufferId, tick: u64, path: PathBuf, text: Rope, crlf: bool, bom: bool, hash: Option<u64> },
    ListDir(PathBuf),
    Open(PathBuf),
}
pub enum RecoveryOp {
    AutoSave { buffer: BufferId, name: String, path: Option<PathBuf>, hash: Option<u64>, text: Rope },
    DeleteAutoSave(BufferId),
    DeleteRecovery(PathBuf),
}
```

Defined next to `Effect` and exported from `aiditor_core` the same way. Each consumer then has one arm per group: the app spawns `files::run(op)`, headless calls it, the recovery forwarders send `op`.

Alternative: keep the flat enum and add `files::run(effect: Effect) -> Result<Msg, Effect>` that hands back what it doesn't handle. It changes fewer producer lines, but `recovery::run` would still take `Effect` and need a catch-all (S10 stays open), and every consumer keeps a three-variant or-pattern for the recovery forward. Grouping costs one wrapping per producer (six sites) and removes both.

### 2. `files::run(op: FileOp) -> Msg`, blocking, in `aiditor-services/src/files.rs`

It is the only place that builds `Msg::Saved`, `Msg::Listed` and `Msg::Opened` from a file operation. Errors: `ListDir` and `Open` map `io::Error` to `"<path>: <err>"` (the app's current text); `Save` passes `files::write`'s `Result<u64, String>` through. The app keeps its messages byte-for-byte; headless gains the path prefix (tooling, not specified).

Logging moves with it: `tracing::info!` for a successful save as today, and a failed save is logged at `info`, not `error`. The core already echoes the error, and every echo lands in `*Messages*`, so the `error!` showed it there twice (H4 of `m0-quick-wins`). The `messages-buffer` spec only requires that warnings and errors that are logged appear; dropping one duplicate log violates nothing. If `m0-quick-wins` has already landed H4, keep its choice.

The app: `Effect::File(op) => { let tx = self.tx.clone(); tokio::task::spawn_blocking(move || _ = tx.blocking_send(files::run(op))); }`. Headless: `Effect::File(op) => files::run(op)` as the reply it sends back into `send`.

### 3. Save carries a `Rope` snapshot; encoding is `buffer::encode(text: &Rope, crlf: bool, bom: bool) -> Vec<u8>`

`Ctx::save` puts `buffer.text().clone()` (ropey shares nodes, O(1)-ish, already done for `AutoSave`), `buffer.crlf` and `buffer.bom` in `FileOp::Save`. `files::run` calls `aiditor_core::buffer::encode` on the blocking pool, then `files::write`. `Buffer::encode` (`buffer.rs:462-475`) becomes that free function with the same body, and the method goes (its only callers are `Ctx::save` and the test at `buffer.rs:594`). The final-newline insert in `Ctx::save` still happens before the snapshot, and `tick` is taken at the same moment, so edits made while the write runs stay unsaved as before.

Alternative: keep `bytes` and only move the encode later. There is no later point in the core; the encode has to happen where the write does.

### 4. Harness: option B, the shared hash in `aiditor_core::buffer::hash`

The harness can't call `files::run`: a dev-dependency on `aiditor-services` builds a second `aiditor-core` for it, whose `FileOp` and `Msg` are different types from the test build's. So the harness keeps its own small executor either way.

- **A** (dev-depend on services, call the std-typed `files::read/list/write`): gains the real atomic write in core tests, but core tests then compile services and Tokio, and CI's check `cargo tree -p aiditor-core > tree.txt && ! grep -E '\b(gpui|tokio) v' tree.txt` (`.github/workflows/ci.yml:39`) would trip, because `cargo tree` includes dev-dependencies by default; it would have to become `cargo tree -p aiditor-core -e normal,build`. Not near-free.
- **B** (chosen): move `hash` (FNV-1a 64, `files.rs:8-14`) into `aiditor-core/src/buffer.rs` as `pub fn hash(bytes: &[u8]) -> u64`, with its doc comment ("fixed because recovery files store it") and the `hash_is_fnv1a` test. `files.rs` imports it; the harness deletes its `DefaultHasher` stand-in (`harness.rs:201-207`) and uses it in `Harness::run`, `Harness::open` and the `left` test helper. The hash belongs in the core anyway: `Buffer::disk` and the recovery header hold it.

The harness's `Save` arm also encodes with `buffer::encode`, so it writes the bytes the real service would. Its `ListDir` arm keeps `e.path().is_dir()` (it follows symlinks, same result as `files::list`).

### 5. Recovery thread takes `RecoveryOp`

`recovery::spawn(dir) -> (mpsc::Sender<RecoveryOp>, JoinHandle<()>)`, `run(dir, op: RecoveryOp)` with three arms and no `_`. The app's `recovery` field type and headless's `Service` alias follow. Forwarding becomes `Effect::Recovery(op) => tx.send(op)`.

### 6. `files::list` stats only symlinks

`entry.file_type()` comes from `readdir` on Linux and macOS without a `stat`. Mark `/` when `ft.is_dir()`, or when `ft.is_symlink()` and `fs::metadata(entry.path())` says directory. A failing `file_type()` counts as not a directory, as a failing `metadata` does today. `lists_sorted_with_directories_marked` already covers file, directory and symlink-to-directory.

### Ordering kept

`Core::autosave` still finds `Effect::Quit` by position and splices `Effect::Recovery(RecoveryOp::DeleteAutoSave(id))` before it; the app and headless still drop the sender and join the recovery thread on `Quit`. The harness's `assert!(!self.quit, "deleted after the quit")` keeps guarding it.

## Risks / Trade-offs

- [The reshape touches every producer and consumer at once] → It is a compile error until all are done; the harness tests, `recovery.rs` tests and `crash_recovery.rs` cover behavior.
- [A message text drifts while moving code] → `ListDir`/`Open` keep the app's `format!("{}: {err}", path.display())` exactly; `Save` passes `files::write`'s result through unchanged.
- [The harness still fakes the save (no atomic write)] → Accepted; covered by `files.rs` tests and the end-to-end test. The hash, the encoding and the error texts are now the real ones.
- [`split-core` moves producers] → Tasks name functions, not lines.
