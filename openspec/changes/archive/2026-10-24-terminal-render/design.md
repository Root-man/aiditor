# Design

## Context

The core owns each terminal's `alacritty_terminal` grid behind `terminal::Terminal` (#223) and sizes it to its smallest window (#224). `frame::window_display` builds a `WindowDisplay` of text lines for every window; for a terminal buffer that is one empty line, since its rope is empty. The frontend (`element.rs`) shapes each display line as one `ShapedLine` and draws a block cursor over it. The frontend has no `alacritty_terminal` dependency.

## Goals / Non-Goals

**Goals:** the core decides everything about what a terminal cell looks like, so harness tests cover colours, attributes, cursor and wide chars; the frontend only places and paints.

**Non-Goals:** damage tracking or caching shaped runs between frames (one screenful shaped per frame, as text windows are); box-drawing glyphs drawn as quads (Zed does this; not asked).

## Decisions

- **A terminal display on `WindowDisplay`.** `WindowDisplay` gains `terminal: Option<display::Terminal>`: `rows: Vec<Vec<Run>>` and `cursor: Option<Cursor>`. A `Run` is `{ col, cells, text, fg: u32, bg: Option<u32>, bold, italic, underline }`; a `Cursor` is `{ row, col, cells, shape: Block | Beam | Underline }`. Text windows leave it `None`; terminal windows keep their mode line and an empty `lines`. Alternative: reuse `Line`/`Span` — spans have no column or font attributes, and widening `Face` would touch every text path.
- **The core resolves colours and batches runs.** `Terminal::display(&Theme)` walks the screen rows (`Line(0..screen_lines)`, so never scrollback), resolves each cell's `Color` (named → theme/palette, indexed → `term.colors()` override else palette/cube/ramp, spec → exact), applies inverse then dim, and appends to the current run while style matches and the column is contiguous. Wide-char spacers are skipped and the wide char's run counts two cells, so a run after a wide char starts at the right column; blank cells on the default background end the run and make none. Alternative: hand alacritty cells to the frontend — it would need the dependency and the theme logic, untestable without GPUI.
- **Cursor from `renderable_content().cursor`.** alacritty already reports `Hidden` when DECTCEM hides it; `HollowBlock` (alacritty's unfocused shape) maps to block. Width is two cells when the cell under it is `WIDE_CHAR`. Hollow-in-unselected is the frontend's, since only it draws by window.
- **Grid placement with `force_width`.** Each run is shaped with `shape_line(text, size, runs, Some(cell))`, which snaps each glyph to `index × cell`, and painted at `col × cell`. Because a run never spans a wide char's spacer (the column jump ends it unless the run is a single wide char — runs are split around wide chars), every glyph sits on its column. The cell width is the same `ch_advance` the core gets in `Msg::Resize`.
- **The IME box follows the selected window.** Unselected terminal windows now paint a cursor too, so `bounds_for_range` takes the selected window's cursor instead of the first one painted.
- **Palette in `[theme]`.** `ansi: [u32; 16]` with VS Code Dark+ terminal defaults, matching the existing VS Code-like theme. `deny_unknown_fields` stays.

## Risks / Trade-offs

- [Display snapshots change shape] → accept the new `terminal: None` field in existing insta snapshots.
- [Glyphs wider than a cell from fallback fonts overflow into the next cell] → same as Zed; they are positioned on the grid, which is what the issue asks.
- [VT parsing and run building on the UI thread during floods] → unchanged trade-off from the spec's output-floods note; building runs is one pass over one screenful per frame.
