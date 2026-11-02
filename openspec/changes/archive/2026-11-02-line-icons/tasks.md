# Tasks

## 1. Core

- [x] 1.1 Add `Icon` and `Prop::Icon` in `buffer.rs`; verify the workspace builds
- [x] 1.2 Add `Line.icons`, record icons in `display::build` and shift them in `wrap_lines`; verify display tests for placement, wrapped rows, a row break, horizontal scrolling and a split face
- [x] 1.3 Verify with tests that motion, search and kill treat an icon as its two spaces
- [x] 1.4 Update the insta snapshots printing `Line` and verify `cargo test -p aiditor-core` passes

## 2. Frontend

- [x] 2.1 Add the Lucide SVGs and `LICENSE` under `crates/aiditor/resources/icons`; verify each file is an SVG with no script or external reference
- [x] 2.2 Add `icons.rs` with the asset source and pass it to `Application::with_assets`; verify a test loads every icon
- [x] 2.3 Lay out icons in prepaint and paint them after the row's text; verify GPUI tests for colour in both themes, default foreground, bounds, text unmoved and marked text

## 3. Docs

- [x] 3.1 Record the property in the data model and Lucide with its licence in `docs/editor.md`; verify the text names the licence
