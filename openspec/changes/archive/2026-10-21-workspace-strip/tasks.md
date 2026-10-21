# Tasks

## 1. Title

- [x] 1.1 Format the frame title as `<buffer>[ *] | <workspace> | <project>` in `frame.rs` and update the `title` doc comment in `display.rs`; verified by `cargo test -p aiditor-core` with updated snapshots (`cargo insta` review of the `title:` lines only)
- [x] 1.2 Extend the title test to cover modification and two workspaces (new, rename, switch back); verified by that test passing
- [x] 1.3 Record in `docs/editor.md` that the workspace strip is the window title (Workspaces: Display bullet and #64's `workspace-list` "until the strip lands"); verified by reading the diff
- [x] 1.4 Add a frontend test that the OS window title is the frame title and follows a new workspace (GPUI test window); verified by `cargo test -p aiditor the_window_title`
- [x] 1.5 Run CI's checks (`cargo fmt --all`, clippy with `-D warnings`, `cargo test --workspace`)
