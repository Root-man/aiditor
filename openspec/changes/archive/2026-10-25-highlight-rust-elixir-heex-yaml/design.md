# Design

## Context

`syntax.rs` maps an extension to a `Grammar` (language plus compiled highlights query) in a `LazyLock`; everything after that (incremental edits, visible-range query, palette lookup) is language-independent.

## Goals / Non-Goals

**Goals:** Rust, Elixir, HEEx and YAML highlighted with their grammars' own queries.

**Non-Goals:** injections (`~H` sigils, Rust doc comments, Markdown in `@doc`), plain `.eex`/`.leex` and other extensions not in the issue, Markdown.

## Decisions

- **crates.io grammars**, each exporting `LANGUAGE` and `HIGHLIGHTS_QUERY` on `tree-sitter-language`: `tree-sitter-rust` 0.24, `tree-sitter-elixir` 0.3, `tree-sitter-heex` 0.8, `tree-sitter-yaml` 0.7. Zed pins git forks of HEEx and YAML for its own query changes; aiditor uses the grammars' own queries, so the published crates do.
- **One query each**, used as shipped; no concatenation is needed.
- **Palette.** Capture names the queries use that no bundled entry or prefix covers and that matter for reading: `module` (type colour, as VS Code colours Elixir modules) and `boolean` (the `constant.builtin` colour). Punctuation, `label`, `escape` and `embedded` stay the foreground colour, as before.

- **EEx templates by their inner extension.** A `.eex` file is highlighted by the extension before it, so `a.html.heex.eex` is HEEx and `a.ex.eex` Elixir. The HEEx grammar parses `<%= %>` itself; in other languages EEx tags parse as errors and only nearby colours suffer.

## Risks / Trade-offs

- [Elixir's query uses internal capture names (`@__name__`, `@comment.doc.__attribute__`)] → they fall back by prefix or stay uncoloured; harmless.
