# Design

## Context

The core has buffers (rope + markers), one window (point, mark markers, `top` line) and settings with a theme. The frontend (`crates/aiditor/src/main.rs`) reads the rope directly and computes rows itself. #11 will replace that drawing with the display model; this change only builds the model in the core.

## Goals / Non-Goals

**Goals:** a pure function from core state and a pixel height to a display model, covering every element the issue lists, snapshot-tested through the harness.

**Non-Goals:** drawing it, splits (one window; #35 adds the tree), the echo area, soft wrap, horizontal scroll, removing or querying properties beyond what the builder needs.

## Decisions

- **Properties as marker pairs.** Each property is `(start, end, Prop)` where `start` is an `Advance` marker and `end` a `Stay` marker in the buffer's existing marker map. Insert and delete already move markers, so properties follow edits with no new edit code; insertion at the edges stays outside (Emacs's default non-stickiness). Alternative: a separate interval list with its own shift logic, which duplicates the marker code.
- **`Prop` enum:** `Face(Face)`, `Invisible(String)` (placeholder), `Line(Decoration)`. `Face { fg: Option<u32>, bg: Option<u32> }` holds colors as 0xRRGGBB like the theme, so the model is self-contained; font family and size can join it later (#122). `Decoration` is `Background(u32)` or `Rule(u32)`, the two the session-buffer decision names.
- **Builder walks visible chars.** From the first char of `top`, it walks the rope char by char: an invisible range is skipped in one step (emitting its placeholder as a fold span), an LF ends the display line, and each char's face is the merge of covering face properties plus the region. Consecutive chars with equal face and kind form one span. Per-char property lookup is a linear scan; visible text is a screenful, so that is fine (shortcut comment).
- **Rows and scrolling.** `rows = floor(height / line_height) - 1` (one row for the mode line, at least 1 text row). `Window::scroll(rows)` picks `top` in buffer lines as today; folds only shrink what is shown, so point stays visible, though `top` can scroll further than needed with folds above point (shortcut comment). Line height lives in `Settings::line_height()` so frontend and core agree.
- **Long lines** are emitted whole; the frontend clips them (truncation per the file-handling decision). The core can't measure text, so it cannot cut at the window's width.
- **API:** `Core::display(height: f32) -> WindowDisplay` (`&mut self` because it scrolls). It returns one window's model; #35 turns it into a list per split.
- **Snapshots:** `insta::assert_debug_snapshot!` of the model where the whole model matters, plain asserts on one field (cursor, mode line, decorations) elsewhere; no custom text format.
- **Marker delete fix.** A marker inside a deleted range longer than its offset underflowed in `Buffer::delete`; property end markers hit that, so the subtraction saturates now.

## Risks / Trade-offs

- [Every property costs two markers, and every edit walks all markers] → fine for the M0 property counts; the existing marker shortcut comment already covers indexing them.
- [Debug snapshots are verbose] → small buffers in tests keep them readable.
