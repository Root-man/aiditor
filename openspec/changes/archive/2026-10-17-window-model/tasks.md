# Tasks

## 1. Core window

- [x] 1.1 Add `window.rs` with `Window` (buffer, point and mark markers, top) and unit tests: an edit before point moves point, mark follows edits, scrolling follows point; `cargo test -p aiditor-core` passes
- [x] 1.2 Replace `Core::current` and `Core::point` with one window; `open` moves the window to the new buffer; existing harness tests pass and a harness test checks scrolling follows point through keys

## 2. Frontend

- [x] 2.1 Drop `Editor::top` and take the first line from `Core::scroll(rows)` in `render`; `cargo clippy --workspace --all-targets -- -D warnings` and `cargo test --workspace` pass

## 3. Docs

- [x] 3.1 Record in `docs/editor.md` that the scroll position is clamped at redisplay until frame size reaches the core; verify the line reads correctly
