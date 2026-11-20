# Tasks

## 1. Repository root on Head

- [x] 1.1 Add `root: PathBuf` (the directory holding `.git`) to `Head` in `crates/aiditor-core/src/lib.rs`, set it in `aiditor_services::files::head()` from the `.git` it found, and fill it in every `Head { .. }` literal (services tests, `display.rs`, `watching.rs` tests); verify with a `files.rs` test that a plain repository and a `.git`-file worktree both report the directory holding `.git`, and `cargo test -p aiditor-services` passing

## 2. Kill ring helper

- [x] 2.1 In `crates/aiditor-core/src/kill_ring.rs`, factor the tail of `Ctx::kill` (take foreign clipboard, `kill_new`, `clipboard_seen`, `Effect::SetClipboard`) into one `Ctx` helper for new standalone entries, keeping `kill`'s join-with-previous behaviour; verify with `cargo test -p aiditor-core kill_ring` passing unchanged

## 3. Commands

- [x] 3.1 Add `copy-relative-path` and `copy-absolute-path` to `crates/aiditor-core/src/builtins.rs` with `ArgSpec { clipboard: true, .. }`: `Buffer is not visiting a file` without a path, the path stripped of `file.head`'s `root` for the relative one (`File is not in a git repository` without a `Head`), then the helper and `echo("Copied <path>")`; verify with harness tests for every scenario in `specs/file-paths/spec.md` (repository, repository below the project root, outside every repository, absolute, yank back and clipboard, `*scratch*` error with nothing copied; the worktree/submodule root is `Head`'s, covered by 1.1's services test) in `cargo test -p aiditor-core`
- [x] 3.2 Record the commands (git repository root from the file's `Head`, error outside a repository, kill ring and clipboard) in `docs/editor.md` next to the kill ring entry; verify the line is there

## 4. Integration

- [x] 4.1 Run `cargo clippy --workspace --all-targets` and `cargo test --workspace` clean on Linux

## Workflow follow-up

- Archive the change after review.
