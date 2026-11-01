# Design

## Context

See proposal.md for motivation. `Syntax` currently owns a flat list of Markdown injections, edited with the host and repainted over it. Both Elixir and HEEx grammars are already bundled. HEEx's injection query distinguishes independent expressions from partial/ending directives that need a shared Elixir parse.

## Goals / Non-Goals

**Goals:** Preserve incremental tree reuse, range clipping, current theme resolution and visible-only paint while supporting nested templates.

**Non-Goals:** Other sigil languages/aliases, new grammars, settings, language servers or asynchronous parsing.

## Decisions

- Give each injection its own children. Sort siblings by their start, skip those outside the visible range and edit and paint recursively, parent first. Combined directives can surround independent expressions, so their end offsets cannot be bisected as a monotonic sequence.
- Use a small `~H` query and HEEx's bundled query. Group only the latter's combined split directives within each template. Independent expressions and distinct sigils keep separate parses to prevent syntax leakage.
- Intersect content with the host's included ranges, so nested templates in Markdown block quotes cannot consume continuation markers. Reuse edited trees only when grammar and included ranges match.
- Clear the host face inside language injections, leaving uncaptured template text in the default face. Markdown inline composition stays as it is; sigil and expression delimiters retain their host faces.

## Risks / Trade-offs

- Split directives can have other expressions between them → keep a sorted range list, paint descendants in order and test interleaving.
- Unicode and boundary edits can invalidate nested trees → compare incremental output with fresh parses, including undo/redo and injection removal.
- Parsing remains synchronous → retain the existing documented shortcut; no service redesign in this issue.
