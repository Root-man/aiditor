# Design

## Context

A terminal buffer's content is `Content::Terminal(Box<Terminal>)`; its `text()` is an empty rope, and `raw_insert`/`raw_delete` refuse any change to it. `Core::char_mode()` decides whether keys and typed text go to the shell. `frame.rs` swaps the window's display for the grid's screen whenever the buffer has a terminal.

## Goals / Non-Goals

**Goals:** copy mode per the delta spec, reusing every text command unchanged.

**Non-Goals:** bold, italic and underline in copy mode (text faces hold only colours), jumping between prompts (OSC 133, a later issue), refreshing the text while in copy mode.

## Decisions

- **The copy text lives beside the grid**: `Content::Terminal(Box<Terminal>, Option<Rope>)`. `text()` returns the copy rope or the empty one, so motion, swiper, the mark and `M-w` read it with no change. Copy mode is "the rope is `Some`". Alternative, a separate text buffer: needs window switching and kill/back-links; rejected.
- **Setting the text bypasses edits**: a buffer method sets the rope, bumps `tick`, clamps every marker and records no undo. Entering puts the current window's point at the cursor; leaving moves every marker to 0 and deactivates the current window's mark. Keeping it outside `insert` keeps "no edit ever touches a terminal" true.
- **Building the text** walks `-history..screen_lines` rows, reusing `Terminal::row`; a row whose last cell has alacritty's `WRAPLINE` flag joins the next without a newline and keeps its trailing blanks. Rows below both the cursor and the last non-blank row are dropped. Point is the cursor row's start plus the chars before the cursor column, capped at that line's end. The alternate grid has no history, so the alternate screen needs no special case.
- **Keys**: `char_mode()` returns `None` in copy mode, so keys and typed text take the normal path. A `vterm-copy-mode` minor mode's keymap binds `RET` to `vterm-copy-mode-done`; `vterm-mode`'s `C-c C-t` binding serves both states, since the major mode stays. The minor mode is pushed and removed with the text.
- **Colours**: the row-run builder behind the live screen's display is shared; `copy_text` turns each run whose colours aren't the defaults into a `Prop::Face` range over the text, clamped to the trimmed line. `set_copy_text` drops every property with the old text. Alternative, drawing copy mode from the grid's runs: would bypass point, region and swiper's highlighting; rejected.
- **Drawing many properties**: `display::build` scans every property for every char. A coloured 10,000-line scrollback can hold tens of thousands, so in a buffer without folds it keeps only the properties overlapping the window's lines, which no other line can reach. Buffers with folds keep the full scan, as before.
- **Mode line**: `display::build` appends `[char]` without the position, or `[copy]` before it, for terminal buffers, as `[deleted]` is appended.
- **Copying** reuses `kill_region(Some(range), true)`, which pushes the kill ring and the clipboard effect.
- **Drawing**: `frame.rs` swaps in the grid only when not in copy mode.

## Risks / Trade-offs

- [10,000 lines of scrollback is built into one rope on entry] → one pass over at most ~10k rows, well under a frame; built once per entry.
- [Point capped at the line's end when the cursor sits past trimmed blanks, e.g. after a prompt's trailing space] → harmless; point lands at the visible end of the prompt.
- [Copy mode per buffer: two windows on one terminal both show the text] → matches Emacs, where the mode is buffer-local.
