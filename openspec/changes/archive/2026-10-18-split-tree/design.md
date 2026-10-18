# Design

## Context

`Core` holds one `Window` in `Core::window`, and every command reaches point, mark and scroll through that field while also borrowing `Core::buffers`. The frontend skeleton draws the buffer itself (not the display model) for the selected window only; #11 replaces it with a renderer that lays windows out by the split tree.

## Goals / Non-Goals

**Goals:** a split tree with ratios, the five commands, per-window point and scroll, and a frame display the core can snapshot.

**Non-Goals:** resizing windows (`C-x ^`, `C-x }`, dragging), balancing, `C-x 4` commands, drawing several windows in the frontend (#11), window configurations per workspace.

## Decisions

- **The selected window stays in `Core::window`; the others live in `Core::others: HashMap<WindowId, Window>`.** `Core::selected` is its id. Selecting swaps the two. Every existing command and test keeps using `core.window` with disjoint borrows of `core.buffers`, so the change touches no command. A slab of all windows plus a `selected_mut()` accessor would borrow all of `Core` and clash with `&mut core.buffers` at dozens of call sites.
- **Binary tree** `enum Tree { Leaf(WindowId), Split { side_by_side, ratio, first, second } }` in `window.rs`. Splitting a leaf replaces it with a split of ratio 0.5 whose first child is the old leaf, which gives Emacs's sizes for repeated splits (25/25/50). Deleting a leaf replaces its parent with the sibling. Order is a depth-first walk, which is top-to-bottom, left-to-right.
- **Layout in pixels**, `Tree::layout(Rect) -> Vec<(WindowId, Rect)>`, first part = `ratio * size`. `Rect` lives in `display.rs` as `{x, y, width, height: f32}`.
  shortcut: boundaries are not snapped to whole lines or cells; #11 snaps them when it draws.
- **`Core::frame(width, height) -> FrameDisplay { windows: Vec<FrameWindow { rect, selected, display }> }`** builds each window's display with the existing `display::build` after scrolling it to its rows. `Core::display(height)` stays as the selected window's display for existing tests and the frontend.
- **A new window copies point, first line and rows**, not the mark (per-window mark, Emacs's mark is per buffer, so a fresh window with no mark is the simplest correct choice).
- **Dropping a window removes its markers** through a `Window::release(buffers)` that `show` already does inline; `show` reuses it.
- **`delete-window` selects the sibling's first leaf.** Emacs selects the most recently used window; MRU order is not tracked, and the sibling is the one that takes the space, which is what Emacs does for the common two-window case.

## Risks / Trade-offs

- The frontend still draws one window, so after `C-x 2` the user sees no split, only `C-x o` switching points. Acceptable: #11 is next and its acceptance criteria cover drawing splits.
