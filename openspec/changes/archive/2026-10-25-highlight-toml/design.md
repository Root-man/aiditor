# Design

## Context

`syntax.rs` maps an extension to a `Grammar` (language plus compiled highlights query) in a `LazyLock`; everything after that is language-independent.

## Goals / Non-Goals

**Goals:** TOML highlighted with its grammar's own query.

**Non-Goals:** files named without a `.toml` extension (`Cargo.lock`, `Pipfile`); they stay plain.

## Decisions

- **`tree-sitter-toml-ng` 0.7** from crates.io (the tree-sitter-grammars organisation's TOML), on `tree-sitter-language` like the other grammars: `LANGUAGE` and `HIGHLIGHTS_QUERY`. The older `tree-sitter-toml` 0.20 depends on an old `tree-sitter`.
- **Plain keys.** The bundled query captures every `bare_key` as `@type` and the whole `pair` as `@property`; the innermost node wins, so `a = 1`'s key would get the type colour while `a.b = 1`'s keys get property. The grammar's query is followed by `(pair (bare_key) @property)`, the later pattern on one node winning, so every key in a pair is a property and only table headers (`[package]`) stay `type`.
- **Palette.** `boolean`, `comment`, `string`, `number`, `string.special` (dates), `operator`, `type` and `property` exist; `punctuation.delimiter` and `punctuation.bracket` have no `punctuation` entry and stay the foreground, as in the other languages.

## Risks / Trade-offs

- [The `pair` capture spans the value too] → whitespace between key and value takes the property colour, which draws nothing; values keep their own innermost colours.
