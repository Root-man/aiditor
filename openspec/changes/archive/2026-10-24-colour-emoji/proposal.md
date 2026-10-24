# Proposal

## Why

On Fedora a starship prompt's 🦀 is blank, in terminals and in file buffers alike (#237). Every emoji falls back to Fedora's only colour emoji font, the COLRv1 `Noto-COLRv1.ttf`, and GPUI 0.2.2's Linux rasterizer (swash 0.2.10, still the same in Zed master) draws COLR v0 layers and colour bitmaps but not COLRv1, so each glyph rasterizes to an empty image.

## What Changes

- At start, the frontend checks the `Noto Color Emoji` font file. When it has neither colour bitmaps (CBDT, sbix) nor COLR v0 layers, GPUI can't draw it, so the frontend drops it from GPUI's font database and emoji fall back to an installed font that has them, such as the monochrome Noto Emoji or Symbola, drawn in the text's colour. This reaches every piece of text the app draws.
- Colour fonts GPUI can draw (Apple Color Emoji on macOS, CBDT Noto Color Emoji on other distros) are left alone.
- A terminal emoji already takes two cells (the core's wide-char runs); a test pins that for 🦀.

## Capabilities

### New Capabilities

### Modified Capabilities
- `frontend-rendering`: emoji are drawn, from another font when the colour font can't be drawn.
- `terminal-buffers`: an emoji is a wide char.

## Impact

- `crates/aiditor/src/main.rs` and a new `emoji.rs` module in the frontend crate.
- Runs `fc-match` once at start to find the colour font's file; no new dependencies.
- `docs/editor.md`: record the finding and the fallback.
