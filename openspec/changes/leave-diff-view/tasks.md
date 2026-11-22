# Tasks

## 1. Leaving the view

- [x] 1.1 Split `diff-quit`'s restore into a `Core` method and call it from `switch_to`, `show_buffer`, `display_buffer`, `split_window`, `delete_window` and `delete_other_windows` as design.md says; verified by the tests in 2.1
- [x] 1.2 Skip window ids gone with the view in the `*Workspaces*` refresh; verified by a test showing `*Workspaces*` from the list while the diff window shows it
- [x] 1.3 Shortcut comment on the server edit shown in a non-current workspace's view

## 2. Tests and docs

- [x] 2.1 Core tests: `C-x b`, `C-x C-f` (pending and open file), `C-x p f`-style `find_path`, a jump, `display_buffer`, `C-x 0/1/2/3` from either window quit and act on the restored layout; editing, saving, `C-x o`, view keys and switching to the list leave it open; a buffer killed by the quit isn't shown — `cargo test -p aiditor-core diff`
- [x] 2.2 Record the audit and the rule in `docs/editor.md`'s diff view section; checked by reading
- [x] 2.3 `cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace` pass
