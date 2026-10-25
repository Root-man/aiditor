# Proposal

## Why

Markdown is the last language the spec lists for highlighting ([#258](https://github.com/Root-man/aiditor/issues/258)); READMEs, docs and agent notes are read and written daily.

## What Changes

- Visiting a `.md` or `.markdown` file parses it with `tree-sitter-md`'s block grammar and highlights it with that grammar's bundled block query, through the existing pipeline: headings, code blocks, list markers, quote markers and thematic breaks.
- `theme.syntax` gains `text.title`, `text.literal`, `text.uri`, `text.reference` and `punctuation.special`, which no existing entry or prefix covers.
- Inline emphasis, code spans, links (the inline grammar) and the language inside fences wait for injections; fenced code is drawn whole in the `text.literal` colour.

## Capabilities

### New Capabilities

### Modified Capabilities
- `syntax-highlighting`: the extension table covers Markdown.

## Impact

- `crates/aiditor-core/Cargo.toml`: `tree-sitter-md`.
- `crates/aiditor-core/src/syntax.rs` (grammar and extensions), `settings.toml` (five palette entries).
- `docs/editor.md`: Language support paragraph.
