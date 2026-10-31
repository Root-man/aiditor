# Design

## Context

The frame builder knows the minibuffer's originating window. Its stand-in window receives a preview only after file loading succeeds. The display builder already layers syntax, properties and region faces while traversing visible characters. Both match faces already exist. See proposal.md for motivation.

## Goals / Non-Goals

**Goals:** Reuse that layering and each command's matcher, keeping highlights transient and scoped to the actual preview.

**Non-Goals:** Change search results, navigation, list rendering, theme definitions or buffer properties.

## Decisions

- Pass an optional borrowed search matcher from the prompt through window display to the display builder. A tiny display matching interface lets Swiper and grep keep their distinct syntax. Computing properties on buffers would leak between windows and require cleanup, so overlays belong in display construction.
- Match complete source lines reached by display traversal and convert byte ranges to character ranges. Apply faces while emitting visible characters, before region faces; wrapping and horizontal cropping then preserve source offsets and anchors. Empty matches have no drawable span.
- Swiper reuses its compiled positive/negative regexps. Grep uses the same existing grep-regex builder as the service, rather than approximating ripgrep smart case with a string uppercase check.
- Keep grep's pending matcher separate from its displayed matcher. Promote it with the first results, retaining the old matcher during debounce and replacement search. A pending file read disables overlays until its preview is installed.
- Defaults: the selected match is the first on the selected hit's line, as both commands select one hit per line; Swiper highlights its whole positive match including ordered-word gaps, and excluded lines receive ordinary faces.

## Risks / Trade-offs

- A very long visible source line costs a full line match → reuse compiled regexps and scan only lines reached by the display traversal, never the entire buffer.
- Region faces can cover match backgrounds → retain the existing region precedence so an active region remains legible.
- External changes can move a grep hit → match current buffer contents using the displayed query, preserving existing navigation behavior.

## Migration Plan

No persisted state changes. Reverting the change restores the previous display.
