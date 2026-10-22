# Tasks

## 1. Core

- [x] 1.1 Add `Effect::Watch(WatchSet)`, `Msg::FilesChanged`, `FileOp::Check` and `Msg::Checked` in `lib.rs`. Compute the watch set at the end of `Core::handle` and emit it when it changed: roots of live workspaces' projects minus `$HOME`, and file buffers' paths. Verify with harness tests: visiting a file adds it, and killing the buffer drops it; a new workspace on a project adds its roots, and killing the workspace drops them.
- [x] 1.2 Add `Buffer::deleted` and the ` [deleted]` mode line. Handle `FilesChanged` (check the visited paths equal to or under a reported path, all of them on a rescan) and `Checked` (set or clear deleted, and `disk = None` when gone). Verify with harness tests for a deleted file, a deleted directory, a rescan, the file coming back, and `C-x C-c` not asking for a clean deleted buffer.
- [x] 1.3 Saving: `save` writes an unmodified deleted buffer, `on_saved` clears `deleted`, and the harness's save and `files::write` write back a gone file. Verify with tests: `C-x C-s` on a deleted buffer, and on a buffer whose file vanished unreported. Update the services `write` test.
- [x] 1.4 Index refresh: a quiet re-walk on a create or delete under an indexed project's roots, or on a rescan, with nothing echoed. Verify with harness tests: create, delete, and a write that starts no walk.

## 2. Service

- [x] 2.1 Add `notify` to `aiditor-services` and write `watcher.rs`: the service thread, kept directories per root, watch diffing per platform, the path filter, canonical path mapping, the debounce, new and gone directories, existence and the rescan flag. `files::run` answers `FileOp::Check`.
- [x] 2.2 Service tests on the real OS backend: one report per write; an atomic save and a later write; create and delete reported; a file outside the roots watched and its watch dropped; `node_modules` ignored (no event, and no inotify watch on Linux); a new directory's files reported.
- [x] 2.3 Wire the app: spawn the watcher in `Editor::new` and route `Effect::Watch`; `aiditor-headless` ignores it.

## 3. Docs and checks

- [x] 3.1 Add a "Built in #80" note under File watching in `docs/editor.md`, and update the `$HOME` "waits for a watcher" line.
- [x] 3.2 `cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace` and the rest of `.github/workflows/ci.yml` pass.
