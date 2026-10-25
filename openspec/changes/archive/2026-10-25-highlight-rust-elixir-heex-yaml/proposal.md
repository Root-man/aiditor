# Proposal

## Why

Only JS/TS files are highlighted ([#256](https://github.com/Root-man/aiditor/issues/256)). Rust, Elixir, HEEx and YAML are the other languages aiditor is used for daily ([#257](https://github.com/Root-man/aiditor/issues/257)).

## What Changes

- Visiting a `.rs`, `.ex`, `.exs`, `.heex`, `.yml` or `.yaml` file parses it with the Rust, Elixir, HEEx or YAML grammar and highlights it with that grammar's bundled highlights query, through the existing pipeline.
- An EEx template (`a.html.heex.eex`, `a.ex.eex`, as Phoenix's generators ship) is highlighted by the extension before `.eex`.
- `theme.syntax` gains `module` (Elixir and HEEx module names) and `boolean` (YAML) entries, which no existing entry or prefix covers.
- `~H` sigils in Elixir files stay one string colour until injections land.

## Capabilities

### New Capabilities

### Modified Capabilities
- `syntax-highlighting`: the extension table covers four more languages.

## Impact

- `crates/aiditor-core/Cargo.toml`: `tree-sitter-rust`, `tree-sitter-elixir`, `tree-sitter-heex`, `tree-sitter-yaml`.
- `crates/aiditor-core/src/syntax.rs` (grammars and extensions), `settings.toml` (two palette entries).
- `docs/editor.md`: Language support paragraph.
