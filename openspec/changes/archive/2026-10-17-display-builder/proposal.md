# Proposal

## Why

The frontend draws raw rope lines, so nothing but the cursor is styled and nothing can be hidden. The design ([Core data model](../../../docs/editor.md#core-data-model), the session-buffer decision in #8) has the core build a display model per window that the frontend only draws, testable without GPUI. Faces, folds and line decorations need a source, and buffers have none yet, so this change also adds minimal text properties.

## What Changes

- Buffers get text properties: a face, `invisible` with a placeholder, or a whole-line decoration on a char range. Ranges follow edits like markers.
- A display builder turns a window, its buffer and the frame height into a display model: the visible lines (each with a height) as styled spans, folds as placeholder spans, line decorations, the cursor, the region and a mode line.
- The theme gains a region color, used for the region overlay.
- Harness tests snapshot the display model with `insta`.
- Not in this change: drawing the model in GPUI (#11), splits (#35), the echo area (#36), soft wrap, removing properties.

## Capabilities

### New Capabilities
- `display-model`: what the core shows for a window: visible lines, spans, folds, decorations, cursor, region and mode line.

### Modified Capabilities
- `text-buffers`: buffers carry text properties that follow edits.
- `settings`: the theme gains a region color.

## Impact

- `aiditor-core`: new `display` module, text properties in `buffer`, `region` in the theme and `settings.toml`, `insta` snapshots in harness tests.
- `aiditor` frontend: unchanged apart from taking the line height from the core.
- `docs/editor.md`: the buffer row lists text properties.
