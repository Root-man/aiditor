# Design

## Context

The columns change retains structured Entry metadata in Snapshot and renders buffer text/properties for navigation. The shared display builds lines per window; mutating that buffer on resize would break two-window fitting and snapshot semantics. Frame 13c is a half-width list beside a 1px divider in a 900px frame; actual splits retain the editor’s existing cell rounding.

## Goals / Non-Goals

Fit presentation only, preserving the active predecessor deltas. No refresh, persistence, new commands or hint changes.

## Decisions

- Build fitted display spans from each visible structured entry after ordinary buffer display establishes row numbers, cursor and decorations. Keep snapshot text and row mappings unchanged. Reuse a single visible-entry traversal for rendering and fitting, including blank rows.
- Wide layout retains 46 location columns and 26 state columns with gaps. Below the room for these plus a 20-column name, omit non-project location and collapse states. Project roots remain while folded/empty project summary suffixes give way. Narrow name/root groups use share_room, with two columns between groups; root shortening reuses the existing leading-ellipsis helper. Existing end-cut helper preserves span faces. This avoids pixel-specific thresholds and follows font/zoom sizing.
- Compact state uses its existing leading glyph; deleted/conflict use `⊘`, failed terminal exit uses `●`, all with their original status face. Preserve multiple applicable state glyphs and their spacing.
- At the per-window frame boundary remove the list mode-line count segment below the same wide-column threshold or if the whole line does not fit; then use normal draw/share_room fitting. Avoid changing every buffer's mode-line representation for one list.

## Risks / Trade-offs

Character counts follow existing mode-line fitting: wide Unicode glyphs can exceed their nominal cell count. Keep the shared documented limitation; do not introduce a second measurement system. Extreme widths may omit indentation/current markers as all content must fit. Native screenshot and Linux checks are unavailable under the authorized automated-only validation scope.
