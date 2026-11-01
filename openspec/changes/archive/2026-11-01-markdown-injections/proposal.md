# Proposal

## Why

Markdown currently highlights only block structure, leaving inline markup and fenced source without their own syntax faces. Issue #332 completes Markdown highlighting using the grammars already bundled.

## What Changes

- Parse Markdown inline regions and recognized code fences as injected trees, incrementally with the host.
- Highlight emphasis, code spans and links with the inline grammar's captures and the existing syntax palette.
- Highlight fenced `rust`, `elixir`, `js`, `ts`, `yaml` and `toml` content with its language's query, limited to the visible ranges. Unknown and unlabeled fences retain the plain literal face.
- Preserve block markup, text-property precedence and existing file-language selection.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `syntax-highlighting`: Markdown inline and fenced-language injections, kept current through edits and queried only where visible.

## Impact

- `crates/aiditor-core/src/syntax.rs`: injected parsing, incremental edits and highlight composition.
- `crates/aiditor-core/settings.toml`: inline emphasis faces, reusing the theme's existing colors and face properties.
- Syntax and display harness tests, plus `docs/editor.md` language-support documentation.
- No new dependencies, frontend APIs or external services.
