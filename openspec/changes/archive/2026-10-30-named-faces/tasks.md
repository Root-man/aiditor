# Tasks

## 1. Faces and themes

- [x] 1.1 Make `buffer::Face` deserializable, add `Face::over`, `FaceRef`, `Prop::Named` and face-named `Decoration`; verify with `cargo check` after 1.2
- [x] 1.2 Add `FaceName`, the new `Theme` and theme resolution in `Settings::parse`; verify with settings tests for overriding one attribute, switching themes, a missing theme, an unknown face in an unselected theme, an unknown attribute and the old `[theme]` table
- [x] 1.3 Rewrite the bundled `settings.toml` to `theme = "default"` and `[themes.default]` with all 41 faces; verify with a test that every issue-listed name is defined
- [x] 1.4 Make syntax entries faces through `syntax.rs`, `buffer.rs` and `display.rs`; verify with an italic comment test

## 2. Reading faces by name

- [x] 2.1 Resolve `Prop::Named` in `display.rs`, read `region` and the wrap marker as faces; verify with display tests
- [x] 2.2 Name faces in `markdown.rs` and `agents.rs`, delete `retheme_sessions` and `map_colors`; verify with tests that a theme change restyles replies, rules, ANSI output and faces sharing a colour
- [x] 2.3 Move `terminal.rs`, the frontend and benches to `Theme::face`; verify with `cargo test --workspace` and `cargo bench --no-run`

## 3. Docs and config

- [x] 3.1 Update `docs/editor.md` wherever it names the old keys or the remap; verify by grep for `code_block` and `` `dim` ``
- [x] 3.2 Back up and move the user's `settings.toml` to `[themes.gruvbox-dark]` and `[themes.gruvbox-light]` from frame 12 and the `docs/editor.md` table; verify `Settings::parse` accepts it with either theme selected
