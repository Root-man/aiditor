# Proposal

## Why

`Cargo.toml` and aiditor's own `settings.toml` are edited often and open uncoloured ([#261](https://github.com/Root-man/aiditor/issues/261)).

## What Changes

- Visiting a `.toml` file parses it with `tree-sitter-toml-ng` and highlights it with that grammar's bundled highlights query, through the existing pipeline: keys, table headers, strings, numbers, booleans, dates, comments and `=`.
- One pattern after the bundled query colours a plain key (`a = 1`) as `property`, as the bundled query already colours dotted keys; table header keys keep its `type` colour.
- No new `theme.syntax` entries: every capture the query uses already has one or falls back to the foreground.

## Capabilities

### New Capabilities

### Modified Capabilities
- `syntax-highlighting`: the extension table covers TOML.

## Impact

- `crates/aiditor-core/Cargo.toml`: `tree-sitter-toml-ng`.
- `crates/aiditor-core/src/syntax.rs`: grammar, extension and tests.
- `docs/editor.md`: Language support paragraph.
