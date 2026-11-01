# Design

## Context

See proposal.md for motivation. `Syntax` owns a block tree, incremental edit state and a visible-range highlight cache. Buffer insert/delete paths already edit that tree, and display requests only drawn ranges. The installed Markdown crate exposes separate block/inline grammars and queries; its parser wrapper demonstrates included ranges for block continuations but does not parse fenced languages.

## Goals / Non-Goals

**Goals:** Add injected trees within the existing syntax lifecycle and reuse all bundled language queries and theme faces.

**Non-Goals:** Other languages' injections, recursive injections, syntax-service offloading, rendered Markdown or hidden markup in file buffers, new dependencies, language aliases beyond the issue's six names.

## Decisions

- Discover inline nodes (including table cells) and labeled code content from the Markdown block tree. Use included ranges in full-buffer byte/point coordinates, excluding block-continuation markers, rather than copying substrings and remapping offsets. This preserves multibyte positions and block quote/list context.
- Keep an injected tree and its included ranges per region. Apply each buffer edit to the host and injected trees/ranges, then rebuild region membership after reparsing the host. Reuse a previous tree only for the same grammar and edited included ranges. Newly created, removed or relabeled regions naturally replace old injections. Use the existing lazy reparse and cache invalidation path.
- Query the host and intersecting injected trees only for requested visible byte ranges. Preserve host faces beneath inline captures; for recognized fences reset included content to the default face before applying the language's captures, so unrecognized tokens do not inherit Markdown's literal color. Keep delimiters and unknown fences literal.
- Use `text.emphasis = { italic = true }` and `text.strong = { bold = true }` in the existing palette. Preserve enclosing face properties when composing inline syntax, so emphasis in headings and nested emphasis retain their context. Code-span and link colors use existing entries. No visual layout change or new design asset is needed.

## Risks / Trade-offs

- Changed injection boundaries can reuse invalid context -> compare grammar and complete edited included ranges before incremental reuse; test delimiter edits and language changes against fresh parses.
- Unicode and block continuations can misalign highlights -> keep absolute coordinates and test nested Markdown with multibyte content.
- Many injections add parsing work -> preserve incremental reuse and limit highlight queries to visible ranges; core-thread parsing remains the existing documented shortcut.

## Migration Plan

No persisted state changes. Reverting restores block-only Markdown highlighting.
