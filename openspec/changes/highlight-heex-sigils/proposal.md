# Proposal

## Why

Phoenix components keep HEEx templates in Elixir `~H` sigils, currently drawn as one string. Issue #333 needs template syntax and embedded Elixir to retain their own faces.

## What Changes

- Highlight uppercase `~H` content as HEEx and its brace and EEx expressions as Elixir.
- Reuse nested injections for standalone HEEx and Elixir in existing Markdown fences.
- Keep template delimiters, incremental editing, visible-range painting and theme behavior intact.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `syntax-highlighting`: nested HEEx/Elixir injections with isolated templates and combined split directives.

## Impact

`crates/aiditor-core/src/syntax.rs`, core display harness tests and `docs/editor.md`. Uses existing grammars and palettes; no dependencies, commands or settings added.
