# Tasks

## 1. Settings

- [x] 1.1 Add `line_height` and `workspace_font_size` to `Settings` and `settings.toml`, reject values out of range, and add `mode_line_height`, `workspace_font_size()`, `workspace_modeline_height` and `text_rows`; verify with settings tests for defaults, 26/22 at 15/13, zoom offset and rejection.

## 2. Core layout

- [x] 2.1 Add `TEXT_INSET`, `WINDOW_TOP` and the divider width, leave the divider in `Tree::layout` and snap stacked text areas; verify with window tests.
- [x] 2.2 Make `window_display` take rows, and inset columns in `frame.rs`, `minibuffer.rs`, `window_cells` and `fit_names`; update the harness, tests and snapshots, and verify with `cargo test -p aiditor-core`.

## 3. Frontend

- [x] 3.1 Remove the root's gaps, size and inset the bars, echo line and rows, and draw text from `text_x` and `WINDOW_TOP`; verify with frontend tests of the edge padding and heights.
- [x] 3.2 Paint the divider and the inactive rule; verify with frontend tests of the gap, the fill and the rule.
- [x] 3.3 Turn ligatures off in `Editor::font` and on the root; verify with a test that the font's `calt` is off.

## 4. Docs

- [x] 4.1 Update `docs/editor.md`: frontend gap, line numbers' note on #353, what is built, and the settings comments in `settings.toml`; verify by grepping for `8 px`.

## 5. Integration

- [x] 5.1 Run `cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace` and the rest of `.github/workflows/ci.yml`.

## Workflow follow-up

- Hand-check against the design in Gruvbox dark and light, and `->` and `!=` in Cascadia Code, at review.
- Archive after review.
