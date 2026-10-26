# Tasks

## 1. Display

- [x] 1.1 Add `Buffer::truncate_lines` and `Buffer::wrap(cols)`; the minibuffer's buffer truncates; verify with a unit test of which buffers word-wrap
- [x] 1.2 Add `display::row_starts` and the optional wrap to `display::build` (split spans, cursor, marker, skipped rows); verify with display tests for code, Markdown, a long word, the cursor, a fold and a short line

## 2. Scrolling

- [x] 2.1 Give `Window` a visual-row top (`top_row`, `cols`) and make `scroll` count visual rows; `window_display` passes the wrap and disables hscroll for wrapped buffers; verify with window tests for the two scrolling scenarios and the existing scroll and hscroll tests

## 3. Command and docs

- [x] 3.1 Add `toggle-truncate-lines`; verify with a harness test toggling and back
- [x] 3.2 Update existing tests that rely on truncation, record the behaviour in `docs/editor.md` (Long lines), and run `cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`
