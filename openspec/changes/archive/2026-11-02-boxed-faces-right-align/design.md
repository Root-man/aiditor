# Design

## Context

See proposal.md for why. `display::build` turns a buffer into spans of resolved `Face`s; `Prop::Invisible` already swaps a range for a placeholder, with cursor, column and search positions handled. `Face` is a `Copy` struct merged by `Face::over`, and themes deserialize it directly. `element.rs` shapes each row as one `ShapedLine`, with span backgrounds painted by `paint_background` at full row height. Word wrap happens twice: `Window::row_starts` from buffer chars (for motion and scroll) and `wrap_lines` from display chars.

The Frame 6c chip has a 1px border, radius 5, a 17px text height in a 21px row, and a border colour that changes with state (`.at`), so the box needs a colour, not a flag.

## Goals / Non-Goals

**Goals:** the two display primitives, tested, ready for the chip and timing tasks.

**Non-Goals:** session use; boxes outside window text rows; measuring wide chars. The chip's 7px inner padding is not added: a box adds no width and hugs its cells, so a glyph touches the border (seen in the drawn check), and the chip task starts and ends the run with a space, one cell standing in for the 7px.

## Decisions

**`Face.boxed: Option<u32>`, `box` in TOML.** Same shape as `fg` and `bg`, so `over`, themes and `FaceRef::Face` work unchanged; no new `FaceRef`. `box` is a Rust keyword, hence the field name and a serde rename. Alternative `bool` plus a theme-wide colour: can't draw the point-on-chip state.

**The frontend strips a boxed run's background and paints one quad per piece.** The run's `bg` at row height would stick out past the inset, rounded box. Each piece paints one quad with its bg and border: left edge and left corners only if the previous piece doesn't share its box colour, right likewise. GPUI's `paint_quad` takes per-side border widths and per-corner radii, so a shared outline needs no grouping type, and a region covering half a chip still draws correctly. Alternative, one group quad plus overlays: more code for the same picture. Quads go after `paint_background` and before the text.

**`Prop::AlignRight` on a real space, handled in the `Prop::Invisible` branch.** As Emacs's `(space :align-to right)`. A display-only property keeps the buffer text honest and motion, search and kill need nothing. Sharing the branch reuses cursor placement, column accounting and `None` search positions. Alternative, the frontend positions a right group by pixels as the mode line does: it can't fall back, as wrapping is the core's, and the harness couldn't test it. Face comes from the same per-char logic, moved to a closure.

**Width.** `cols` is `wrap.width + 1` when wrapped, else `shown.len() - 1`; `shown` of `usize::MAX` length means unknown width and draws one space. The fit test is against the row width (`wrap.width` when wrapped, `cols` otherwise) so that a line the display pads is one that `Window::row_starts`, counting buffer chars, also keeps on one row. Padding then goes up to `cols`, past the column wrapping keeps for the `\` marker, so `Line.aligned` tells `wrap_lines` to leave it unbroken. A wrapped top line shown from `from` counts those chars as already used.

**Rest of line.** Walks from the property's end with the existing `stop` closure, adding plain runs and fold placeholders, until a newline. Only runs at a property, so lines without one cost nothing beyond the existing per-char `find_map`.

## Risks / Trade-offs

- [A glyph from a fallback font, such as `▤`, can be wider than a cell] → right-aligned text lands that many px off; the chars counted are still right. Same limit the existing wide-char shortcut records; measure when the display does.
- [`cols` is whole cells] → text ends up to a cell short of the inset, moving in cell steps on resize. Consistent with every other column-based layout here.
- [Adjacent chips with no gap share an outline] → the chip task keeps a space between them, as frame 6c draws.
- [Nothing uses these yet] → no user can see them without the later tasks; tests carry the acceptance, with a one-off drawn check during this change.
