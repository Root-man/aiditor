# Tasks

## 1. Group headings and folding

- [x] 1.1 Add `Row::Group { root, staged }`, record it for each group heading in `render_diff_list`, and draw the heading as mark + name padded to 7 + count right-aligned to 3; update the expected list text in diff.rs tests and verify `cargo test -p aiditor-core diff` passes
- [x] 1.2 Make `DiffList::folded` hold `(root, Option<staged>)`, skip a folded group's files, and have `diff_toggle` fold the nearest root or group heading at or above point; add tests for folding one group (other kept, point on its heading, count kept) and for a group fold surviving a refresh and a root fold/unfold, and update `tab_folds_a_root_until_unfolded_across_refreshes` to start on the root heading
- [x] 1.3 Add a test that a 1-file and a 3-digit group's counts end in the same column
- [x] 1.4 Update docs/editor.md "Look as built" for group fold marks, per-group `TAB` and aligned counts

## 2. Debounce

- [x] 2.1 Set `REDIFF` to 50 ms and change docs/editor.md's "300 ms after the shown buffer's last edit" to 50 ms; verify the re-diff tests still pass

## 3. Checks

- [x] 3.1 `cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D warnings` and `cargo test --workspace` pass
