# Tasks

## 1. Preview and jump to a file position

- [x] 1.1 Add `Preview::FileAt(path, line, col)`; generalise `preview_point(p)` to `preview_at(buffer, p)` and resolve `FileAt` like `File` (visiting buffer, else the peek read). Verify the swiper and minibuffer preview tests still pass.
- [x] 1.2 A pending jump in the core, applied by `Msg::Opened` after `visit` when the path matches. Verified by the jump test in 2.4.

## 2. The command

- [x] 2.1 The harness answers `Effect::Grep(Search)` with an in-process regex search over `walk_files` (one batch, done) and records `Cancel`; `root_names` takes root paths.
- [x] 2.2 `ProjectGrep` state in the core and its poll from `poll_completion`: debounce timer, 3-char minimum, search numbers, cancel when the prompt is gone; a minibuffer helper that replaces or extends a filtering prompt's items. Verify with harness tests for the debounce (no search at 100 ms, one at 200 ms), too-short text emptying the list, and `C-g` cancelling.
- [x] 2.3 `Msg::Grepped` handling: drop other numbers, first answer replaces, later ones extend, an error echoes and empties. Verify with tests for the label format across two roots, old hits staying until the new answer, a late answer dropped, and an invalid pattern.
- [x] 2.4 `project-grep` command, its spec (all-pass filter, `FileAt` preview) and `C-x p g` in `keymap.toml`; `RET` jumps. Verify with the key-script test "search, move, jump" (visited and unvisited file), the preview test (moves with `C-n`, `C-g` restores) and the history test.
- [x] 2.5 Record the command in `docs/editor.md` (Search and navigation), and drop the harness and core `shortcut:` comments about #74.

## 3. Checks

- [x] 3.1 `cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace` and the rest of `.github/workflows/ci.yml` pass.

## Workflow follow-up

- Archive the change after review.
