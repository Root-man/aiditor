# Proposal

## Why

Lines wider than a window are drawn whole and clipped at the window edge, so `C-e` on a long line puts the cursor off screen (#153). M0 truncates long lines, so point's line has to scroll sideways, as Emacs's `auto-hscroll-mode` set to `current-line` does (decided in #45).

## What Changes

- Each window keeps a horizontal scroll offset in columns that moves just far enough to keep point visible, and is 0 whenever point's column fits the unscrolled window.
- The offset applies to point's display line only; the other lines stay truncated at the window edge.
- The minibuffer line scrolls the same way, its width being the frame's less the prompt.
- A click on the scrolled line lands on the clicked char.

Defaults taken: no `hscroll-margin`, no recentering (`hscroll-step` 0); a column is one char, so tabs and wide chars count as one; a display asked for without a width does not scroll.

## Capabilities

### New Capabilities

### Modified Capabilities
- `display-model`: adds horizontal scrolling of point's line.

## Impact

`aiditor-core`: `window.rs` (the offset), `display.rs` (cutting point's line), `lib.rs` (window width in columns, click column). The frontend draws the lines as before.
