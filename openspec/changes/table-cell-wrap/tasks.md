# Tasks

## 1. Setting

- [x] 1.1 Add `table_width` to `Settings` and the bundled `settings.toml` (100); verify settings tests for default, override and `-1`/string rejection pass

## 2. Drawing

- [x] 2.1 Change `markdown::render`'s `tables: bool` to `Option<usize>` and pass `Some(settings.table_width)` from `render_stream` (and the debug check), `None` elsewhere; verify the crate builds and existing table tests pass
- [x] 2.2 Add `last` to `Decoration::Table`, carried through `display.rs`; verify display tests pass
- [x] 2.3 In `draw`, share column widths under the cap and wrap cells at words onto more lines per row, alignment per line, `last` only on a row's last line; verify new markdown tests for wrap, broken words, alignment, rules flags, a fitting table and a streaming wide table
- [x] 2.4 Draw the bottom rule only on `last` lines in `element.rs`'s `table_rules`; verify its unit test covers a non-last line

## 3. Docs

- [x] 3.1 Update the tables paragraph and the settings in `docs/editor.md`; verify it mentions `table_width`, wrapping and the bottom rule
- [x] 3.2 Run `cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D warnings` and `cargo test --workspace`; all pass
