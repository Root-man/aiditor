# Tasks

Locations are item names first; line numbers are as of `8fa1ddd` and will have moved after `split-core`. See design.md for each decision.

## 1. Shared hash and encoding in the core

- [x] 1.1 Move `hash` (FNV-1a 64, `crates/aiditor-services/src/files.rs:8-14`) to `crates/aiditor-core/src/buffer.rs` as `pub fn hash`, keeping its doc comment, and move the `hash_is_fnv1a` test with it; `files.rs` uses `aiditor_core::buffer::hash`. Verify `cargo test -p aiditor-core hash_is_fnv1a` and `cargo test -p aiditor-services` pass.
- [x] 1.2 Delete the harness's `DefaultHasher` stand-in `fn hash` (`crates/aiditor-core/src/harness.rs:201-207`) and use `buffer::hash` in `Harness::run`, `Harness::open` and the `left` test helper. Verify `cargo test -p aiditor-core` passes.
- [x] 1.3 Turn `Buffer::encode` (`buffer.rs:462-475`) into `pub fn encode(text: &Rope, crlf: bool, bom: bool) -> Vec<u8>` in `buffer.rs`, same body, and update its test in `decodes_and_encodes_bom_and_line_endings` (`buffer.rs:594`) to call `encode(b.text(), true, true)`. `Ctx::save` calls `encode(buffer.text(), buffer.crlf, buffer.bom)` for now. Verify `cargo test -p aiditor-core` passes.
- [x] 1.4 Fix `docs/editor.md` File handling (#46) bullet (line 132): the hash is FNV-1a 64 (not `DefaultHasher`), and saves encode on Tokio's blocking pool. Verify by reading the paragraph.

## 2. Grouped effects and the one file executor

These steps change types every crate uses, so they land together; the workspace compiles again at 2.6.

- [x] 2.1 In `crates/aiditor-core/src/lib.rs` next to `pub enum Effect` (`:191-224`), add `pub enum FileOp { Save { buffer, tick, path, text: Rope, crlf, bom, hash }, ListDir(PathBuf), Open(PathBuf) }` and `pub enum RecoveryOp { AutoSave { buffer, name, path, hash, text }, DeleteAutoSave(BufferId), DeleteRecovery(PathBuf) }`, and reduce `Effect` to `File(FileOp)`, `Recovery(RecoveryOp)`, `LoadSettings`, `SetClipboard`, `Quit`. Move each variant's doc comment to its new home; update the doc references in `Msg::Saved`/`Listed`/`Opened` (`:68-85`), `Recovered` (`:104`) and `command.rs:28`.
- [x] 2.2 Update the producers: `Msg::AutoSave` arm of `Ctx::dispatch` (`:575`), `Ctx::find_file` (`:1256`), `Ctx::recover_buffers` (`:1342`), `Ctx::save` (`:1373`, now `text: buffer.text().clone(), crlf: buffer.crlf, bom: buffer.bom`, with the final-newline insert and `tick` still taken first), `Core::autosave` (`:1968-1978`, splice `Effect::Recovery(RecoveryOp::DeleteAutoSave(id))` before `Quit` as now), `Core::poll_completion` (`:2015`). Verify by `cargo check -p aiditor-core --lib`.
- [x] 2.3 Update `Harness::run` (`harness.rs:110-185`) to match `Effect::File(FileOp::..)` and `Effect::Recovery(RecoveryOp::..)`; its `Save` arm writes `buffer::encode(&text, crlf, bom)`. Update `edits_during_a_save_keep_the_recovery_file` (`harness.rs:2784`) to destructure `Effect::File(FileOp::Save { buffer, tick, .. })`. Verify `cargo test -p aiditor-core` passes.
- [x] 2.4 Add `pub fn run(op: FileOp) -> Msg` to `crates/aiditor-services/src/files.rs`: `Save` encodes with `aiditor_core::buffer::encode`, calls `write`, logs success with `tracing::info!(path, "saved")` and failure at `info` too (H4; keep `m0-quick-wins`' choice if it landed), returns `Msg::Saved`; `ListDir` and `Open` map errors with `format!("{}: {err}", path.display())` into `Msg::Listed`/`Msg::Opened`. Add one test that `run(FileOp::Open(missing))` gives `Ok(None)` and `run(FileOp::ListDir(file))` gives an error starting with the path. Verify `cargo test -p aiditor-services files` passes.
- [x] 2.5 `recovery::spawn` returns `mpsc::Sender<RecoveryOp>`; `recovery::run` takes `RecoveryOp` with three arms and no catch-all (remove `_ => Ok(())`, `recovery.rs:82`); update its doc comment (`:15`). Update the tests (`recovery.rs:139-212`): `run_all` takes `Vec<RecoveryOp>`, the core's effects are filtered to `Effect::Recovery(op)`, and the deletes are `RecoveryOp::DeleteAutoSave`/`DeleteRecovery`. Verify `cargo test -p aiditor-services recovery` passes.
- [x] 2.6 Callers. `Editor::handle` (`crates/aiditor/src/main.rs:121-183`): `Effect::File(op)` spawns `files::run(op)` on `tokio::task::spawn_blocking` and `blocking_send`s the reply; `Effect::Recovery(op)` sends `op` to the recovery thread; `Quit` still drops the sender and joins first; the `recovery` field's type follows. Headless `send` (`aiditor-headless.rs:51-86`): `Effect::File(op) => files::run(op)`, `Effect::Recovery(op)` forwards, `Service` alias (`:18`) uses `Sender<RecoveryOp>`. Verify `cargo build --workspace` and `cargo test -p aiditor-services --test crash_recovery` pass.

## 3. Directory listing stats only symlinks

- [x] 3.1 In `files::list` (`files.rs:28-43`), mark `/` from `entry.file_type()`: a directory, or a symlink whose `fs::metadata(entry.path())` is a directory; a failing `file_type()` counts as not a directory. Verify `lists_sorted_with_directories_marked` passes.

## 4. Integration checks

- [x] 4.1 Verify `grep -rn "Msg::Saved\|Msg::Listed\|Msg::Opened" crates/*/src --include=*.rs` shows construction only in `files::run`, the harness and tests, and that `recovery::run` has no `_ =>` arm.
- [x] 4.2 Verify `cargo fmt --all --check`, `cargo clippy --all-targets -- -D warnings` and `cargo test` pass on Linux and macOS, and CI's `cargo tree -p aiditor-core` check still finds no gpui or tokio.

## Workflow follow-up

- Archive the change after review; no main specs change (`skip_specs: true`).
