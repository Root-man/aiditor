# Design

## Context

`syntax.rs` owns grammars, incremental trees, Markdown injections and visible-range painting. It currently ignores locals queries and the JavaScript highlights query's local predicates. JavaScript and TypeScript export locals queries; the Rust crate does not.

## Goals / Non-Goals

**Goals:** Preserve definition faces through lexical binding, offscreen definitions and existing injections without adding a dependency or changing display APIs.

**Non-Goals:** Semantic name resolution, LSP, additional languages or a complete Rust binding analyzer. Rust starts with identifier parameters, closure parameters and simple let bindings.

## Decisions

- Compile optional locals queries alongside highlights. Extend JavaScript's locals query with TypeScript's, following existing highlight composition. Add a small Rust query with non-inheriting function scopes and inheriting blocks/closures.
- Build and cache a whole-tree binding index lazily once per parse. Process scopes and definitions in source order, resolving each reference to the nearest eligible definition. Honor `local.scope-inherits` and `local.definition-value`.
- Query definition highlight captures once when building the index, preserving current capture precedence. Keep capture identities rather than faces so palette changes use current settings. Query visible highlights as before and substitute resolved definition captures, respecting local predicates.
- Keep the index with each tree, including each Markdown injection, and invalidate it when trees are reparsed. The existing final-runs cache continues to key on ranges and palette.

## Risks / Trade-offs

- Whole-tree binding discovery adds work after edits → cache per tree, leave repeated draws and scrolling with visible highlight queries. Parsing already runs on the core thread; retain and document that limitation.
- Queries are syntax heuristics, not compiler resolution → keep scope explicitly limited to supported patterns and test shadowing, initializers, scope exit, Unicode and separate functions/fences.
- Query composition or predicates regress existing colors → test JS/TS/TSX and retain existing capture precedence tests.
