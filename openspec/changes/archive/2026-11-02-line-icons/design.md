# Design

## Context

See proposal.md for why. A `Prop` on a char range becomes `Span`s in `display::build`; `wrap_lines` re-splits them into rows, the region and search overlays split them by face. `FrameElement` shapes each `Line` in one `ShapedLine` and `x_for_index` gives its boxes their bounds. GPUI 0.2.2's `Window::paint_svg` rasterizes a path from the app's `AssetSource` as an alpha mask and tints it. `Application::new()` has no asset source.

## Goals / Non-Goals

**Non-Goals:**
- Using an icon in a session row (the tool-header task).
- Keeping an icon visible under the block cursor. It shows only on an icon's cell, in rows point rarely rests on.
- Icons in the mode line, the echo line or terminal rows.

## Decisions

**`Prop::Icon(Icon)` on the two spaces.** The text really is two spaces, so motion, search, kill and copy need no code. `Icon` is a plain enum in `buffer.rs`; the core names icons and knows nothing of SVGs.

**`Line.icons: Vec<(usize, Icon)>`, not a `Span` field.** Faces split spans anywhere, so a span field would need every split (region, search, wrap, marked text) to keep the icon whole. A column on the line survives all but wrapping, and `wrap_lines` shifts it in one place. `build` records an icon at its first cell when its range is two chars and both cells are within the shown columns; `wrap_lines` drops one whose cells land on different rows. Snapshots printing `Line` gain `icons: []`.

**Colour from the shaped runs.** The frontend finds the `TextRun` at the icon's byte and takes its colour, so face, region, overlays and theme all count with no new core field, and the colour is the run's by construction.

**Position from the shaped line.** `x_for_index` of the two spaces, as boxes do, so a wide fallback glyph earlier in the row can't misplace it. Marked text moves columns at or past it by its char count.

**Assets in the binary.** `include_bytes!` in a small `AssetSource` in `icons.rs`, passed to `Application::with_assets`. A `match` from `Icon` to path keeps the set exhaustive. Alternative `rust-embed`: a dependency for six files.

**Lucide, ISC.** Zed uses it too, it is stroke-only at a 24 unit grid and it rasterizes to a clean mask. Picks: `book-open`, `search`, `pencil`, `terminal`, `globe`, `wrench`, saved under the set's names with the licence as `LICENSE` beside them.

## Risks / Trade-offs

- [The design canvas isn't readable from here, so the icons' weight and size are not compared with frame 6] → Lucide's 2-unit stroke at font size; the hand check judges it.
- [Nothing in the app paints an icon until the tool-header task] → the manual check uses a throwaway patch.
