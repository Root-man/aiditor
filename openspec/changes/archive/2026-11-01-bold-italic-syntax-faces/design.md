# Design

## Context

See proposal.md for motivation. #332 already changed Theme.syntax to Face values, migrated bundled and personal palettes, and propagated full faces through display spans into GPUI font runs. TOML tables merge recursively before deserialization; false values therefore disable inherited styles without changing Face.over's additive layering.

## Goals / Non-Goals

**Goals:** Make the existing contract explicit and test its complete settings-to-font path.

**Non-Goals:** Change bundled syntax styling, add compatibility with bare colours, or change text-property face layering.

## Decisions

- Reuse recursive settings merging and Face values. A parallel syntax type or renderer would duplicate working behavior.
- Extend existing core tests for display faces and named-theme overrides. Add a GPUI test that reloads settings on an open source buffer and checks resulting font runs for both style states and retained foregrounds.
- Retain the bundled default's existing colours and normal comment/keyword styles. Gruvbox comments stay italic and syntax keywords stay normal unless overridden.
- Personal themes already contain only face tables; verify their syntax sections rather than rewrite them.

## Risks / Trade-offs

- Test-platform font runs cannot prove native glyph appearance → ask for the exact native check if native computer use is unavailable.
- Bare colours remain invalid → explicitly test selected and unselected themes and settings retention.

## Migration Plan

No further migration is needed for bundled or personal settings: #332 already applied it. No external configuration writes.
