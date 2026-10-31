# Design

## Context

`WindowDisplay.mode_line` is a `String` that `display::build` formats, `frame()` appends `[unwatched]` to, and `element.rs` shapes in one face. About 40 test sites and 8 snapshots read it. `display::build` doesn't know the window's pixel width or whether it is selected; `frame()` does. See proposal.md for motivation.

## Goals / Non-Goals

**Goals:** everything the issue asks to test is decided in the core, so display tests reach it; the frontend only draws spans.

**Non-Goals:** a user-configurable mode line format; new contents; a per-font width measure.

## Decisions

**Two stages.** `WindowDisplay.mode_line: ModeLine` holds `left` and `right: Vec<Segment>`, where a `Segment` is `Vec<(String, FaceName)>`, plain text being `FaceName::ModeLine`. `display::build` fills it, a `match` on buffer type supplying each type's segments. `frame()` adds `[unwatched]`, then `ModeLine::draw(cols, selected, theme)` gives `FrameWindow.mode_line: ModeBar { left, right: Vec<Span> }`, the frontend's only input. Alternative: one type holding spans with separators, which would make `frame()` edit flattened text to add `[unwatched]`; or separators and cuts in the frontend, which display tests can't reach.

**Names, not resolved faces, in segments.** Tests read which face a segment wants, and `draw` applies the theme: `theme.face(name).over(bar)`, or the bare inactive face. `Span` is reused for the output, so the frontend's run code applies.

**`ModeLine::text()`** joins segments with ` │ ` and groups with two spaces, for tests: `-- a.txt  1:0` stays as it was, which keeps most assertions as they are.

**Truncation in columns.** `cols` is the window's width less both insets over the cell width, with a two-column gap, like `fit_names`. Its room rule moves to one function both call. Cuts run across spans, so the `…` takes the face of the span it lands in.

**Colours now.** The issue's "drop every segment's colour" needs some to drop, so state gets the design's faces. The session mode stays plain; colour by risk is its own task.

## Risks / Trade-offs

- [Wide chars count one column, so a CJK or emoji name can overdraw the right group] → the same shortcut as text scrolling, with a `shortcut:` comment; both groups are clipped to the window.
- [A terminal's `%%` is always red until its own mode line task] → that task drops it.
- [Cuts can end on a separator, `-- a  │…`] → accepted; dropping whole segments would hide more.
