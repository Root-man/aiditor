# Design

## Context

`Core::frame` builds `FrameDisplay` for the size the frontend last reported with `Msg::Resize`; that size is the `FrameElement`'s bounds, the flex-1 area of the editor's column. The frontend already builds the debug stats line, hint and echo area as divs in that column.

## Goals / Non-Goals

**Goals:** a one-row bar outside the split tree, predictable truncation testable in the core.

**Non-Goals:** other workspaces, agent dots, clock, new theme keys, a separate font size.

## Decisions

- **Truncate in the core.** `frame()` cuts the names to `frame_size` columns (width / cell width), so the rule is a pure function tested through the harness. GPUI's flex shrink would cut by base size, not keep both names. The bar is a sibling of `FrameElement` in the same padded column, so their widths match.
- **Layout in the frontend.** The bar is a `div` before the `FrameElement`; flex layout gives the windows the remaining height, and `Msg::Resize` already reports it, so nothing in the core lays out the bar.
- **Existing theme colours.** Background `mode_line_inactive`, separator `dim`, names in the foreground. The design's yellow workspace has no theme key; adding one is left for a theme change.
- **Fields.** `FrameDisplay` gains `project` and `workspace` strings; the title keeps its own format.

## Risks / Trade-offs

- [Column count by chars] wide characters take two cells → the row clips with `overflow_hidden`, never wraps.
