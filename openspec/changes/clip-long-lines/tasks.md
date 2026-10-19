# Tasks

## 1. Cut lines in the display model

- [ ] 1.1 In `window_display` (`crates/aiditor-core/src/lib.rs`), take point's column from `buffer.line_col(point)`, call `window.hscroll` before `build` with a `shortcut:` comment about folds before point, pass `left..left + width` (width `cols + 1` saturating, `usize::MAX` for `cols == 0`) and delete the post-cut. Keep the tab/wide-char shortcut comment. Verify `cargo test -p aiditor-core` compiles once 1.2 lands.
- [ ] 1.2 In `display::build`, take `shown: Range<usize>`. Count columns per line, skip before `left` on point's line, and at the limit jump to the newline or the next fold start, as design.md "The loop" says. Report the cursor in shown columns and replace the `shortcut:` comment at `display.rs:82` with one about soft wrap (#112) only. Verify all existing insta snapshots pass with no `.snap` change.
- [ ] 1.3 Update `point_s_line_scrolls_sideways_to_keep_point_visible` (`harness.rs`) to expect `long[..41]` for unscrolled lines, and check that `the_minibuffer_line_scrolls_sideways` and `a_click_on_the_scrolled_line_lands_on_the_clicked_char` pass unchanged. Verify with `cargo test -p aiditor-core`.
- [ ] 1.4 Add harness tests for the spec scenario "A line far wider than the window": a 10 000-char line plus a next line at 20 columns (`h.frame(160., 600.)`), point at the end gives the last 19 chars from column 9 981 with the cursor at 19, and point on the next line gives the first 21 chars. In the same tests, cover a `Prop::Line` decoration on the long line still being present, a fold over the long line's newline still joining it to the next line, and `cols == 0` (a frame before any resize) cutting nothing. Verify with `cargo test -p aiditor-core`.
- [ ] 1.5 In `docs/editor.md`'s Long lines bullet, add that the display model holds only the shown columns, so a long line costs a window's width per redisplay. Verify the bullet still reads right.

## 2. Integration

- [ ] 2.1 Run `cargo fmt --all --check`, `cargo clippy --all-targets -- -D warnings` and `cargo test` (including the frontend test `the_cursor_is_drawn_over_the_whole_line`) on Linux, and confirm `aiditor-core` still has no gpui or tokio dependency (`cargo tree -p aiditor-core`).

## Workflow follow-up

- Run the same checks on macOS before merging.
- Verify, commit, archive and ship per the implement skill.
