# Tasks

## 1. Core

- [x] 1.1 Add `ignore: Vec<String>` to `Project` (serde default, skipped when empty) and to the ad-hoc and command-built projects; verify with a `project.rs` test that `ignore = ["*", "!notes/"]` parses in order, round-trips, and that `sync` leaves the key alone
- [x] 1.2 Carry the patterns in effects: `FileOp::Index { ignore }`, `GrepSearch { ignore }`, `WatchSet.roots: Vec<(PathBuf, Vec<String>)>`; update `index_project`, project grep, `watch_set` and the harness; verify `cargo test -p aiditor-core` passes and a `watching.rs` test checks the watch set carries a project's patterns

- [x] 1.3 Re-walk every indexed project quietly when projects reload; verify with a `watching.rs` test that a reload starts one walk and echoes nothing

## 2. Services

- [x] 2.1 Add compiled patterns to `files.rs` (one `Gitignore` per pattern, last matching the path or a parent decides) and make `walker`/`walk`/`index` take the root's patterns; verify with `files.rs` tests for plain patterns, the allow-list with a later exclude, and `!target/` not overriding `.gitignore`
- [x] 2.2 Check the patterns in `projects::check`; verify with a `projects.rs` test that `ignore = ["src/[z-a]"]` rejects the file naming it
- [x] 2.3 Apply the patterns in grep; verify with a `grep.rs` test that an allow-list limits hits to `notes/`
- [x] 2.4 Key the watcher's roots by (root, patterns) and use each root's patterns for its walk, listings and new directories; verify with `watcher.rs` tests that an ignored file isn't reported and that an allow-list keeps (and on Linux watches) only the root and `notes/`

## 3. Docs and checks

- [x] 3.1 Record the decisions in `docs/editor.md` (Projects: file format and the "Watching and indexing a large project" bullet); verify the section reads right
- [x] 3.2 Run `cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D warnings` and `cargo test --workspace`; verify all pass
