# Design

## Context

The core's one nucleo matcher serves every fuzzy prompt; the frame exposes candidate and annotation strings separately. The frontend already renders theme-aware text runs for workspace spans. `completion-match` is defined but unused. See proposal.md for motivation.

## Goals / Non-Goals

**Goals:** share one implementation across fuzzy prompts, bound matching work to the ten shown rows, and preserve Unicode text and existing selection/annotation styling.

**Non-Goals:** custom regex filters (user-confirmed), changing ranking, new settings or new theme faces.

## Decisions

- Add per-candidate UTF-8 highlight ranges to the existing display model; preserve strings for callers, previews and annotations. This avoids changing the command/provider contracts or duplicating candidate strings as spans.
- Derive indices using the snapshot's column pattern and original matcher columns, with a reusable scratch matcher owned alongside nucleo. Do not reparse the live input, since its asynchronous results may still show an older pattern.
- Sort and deduplicate atom indices, then map them to complete grapheme byte ranges with the installed `unicode-segmentation` crate, now a direct dependency. Char offsets alone fail after decomposed accents and joined emoji.
- Render normal and highlighted portions with the existing `text_run` helper and `StyledText`. Leave the row background and separate dimmed annotation element in place. Resolve the face every frame for theme reloads.
- Custom-filter candidates carry empty highlight ranges; file prompts use the existing basename pattern and candidate columns.

## Risks / Trade-offs

- [Repeated scoring adds work] → Compute indices for visible rows only and reuse scratch matcher storage.
- [Stale asynchronous results] → Use the displayed snapshot's pattern, not pending input.
- [Unicode indexing mismatch] → Test decomposed accents, multibyte text and joined emoji; use grapheme boundaries matching nucleo's representation.

## Structural acceptance checklist

- [x] Existing command, preview, annotation and selection contracts remain intact.
- [x] No new configuration or theme face; existing installed segmentation dependency reused.
- [x] Formatting, clippy, workspace tests and remaining CI checks pass.
