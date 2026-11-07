# Design

## Context

See proposal.md for the motivation. How it works today:

- `Core::output` (`agents.rs`) inserts each chunk into the buffer as it arrives and adds its chars to `Message.len`. The buffer text is the only copy of a message's markdown.
- `render_stream` runs at most once per frame and calls `render::render`, which replaces the streamed block's properties with those from `markdown::render(text) -> Vec<(Range, Prop)>`. The text itself is never changed.
- Tool calls and permission blocks already keep their source and rewrite their text with `render::rewrite`. It diffs the old and new text by line, so point on an unchanged line stays where it is.
- Display: `wrap_lines` already keeps a line on one row when its `aligned` flag is set, and `cut_spans` cuts spans at a width. `Window::hscroll` computes the sideways offset for point's line when the buffer truncates lines.
- Frontend: `element.rs` paints `Decoration::Border` and `HRule` as 1 px quads across a row.

## Goals / Non-Goals

**Goals:**
- Message tables become real text with painted rules. Nothing else in a message renders differently.
- Today's per-chunk insert, once-per-frame render contract (session-buffers "Streamed messages re-render once per frame") stays as it is.

**Non-Goals:**
- A source-to-shown position map, or copying a message's markdown.
- Pixel-accurate widths for wide characters and emoji.

## Decisions

### 1. The stream keeps its source, and chunks still go in at once

`Stream` gains `source: String`. `output` still inserts each chunk at the block's end, as the spec requires, and also appends it to `source`. When the block is dirty, `render_stream` calls `markdown::render(&source)`, `rewrite`s the block's range with the returned text, then adds the properties. `Message.len` is then corrected by the change in the block's length.

Between frames the block is therefore the rendered prefix followed by raw chunks, and each frame redraws it whole. Because `rewrite` diffs by line, only the changed lines are touched: the tail, plus a table's lines when its columns widen.

*Alternative:* insert nothing until the frame. Rejected because it changes the streaming spec and the bookkeeping for each chunk.

`trim_message` trims trailing newlines from `source` as well as from the buffer, so the final render agrees with it. Replay (`session/load`) goes through `output`, so it gets the same handling.

### 2. `markdown::render` returns the shown text

The signature becomes `render(source, thought, body, tables, is_file) -> (String, Vec<(Range, Prop)>)`, with offsets into the returned text.

With `tables` off (thoughts, tool bodies, the echo), the text returned is the source, so those callers ignore it and see no change. With `tables` on (streamed messages only), each `pipe_table` node in the block tree is replaced by its drawn lines. Every byte outside a table is copied through unchanged, along with its existing props, shifted by how much the tables before it grew or shrank.

Link faces for file words and bare URLs are found on the returned text, so they work in cells too.

*Alternative:* a separate table pass after `render`. Rejected: two passes over the same parse would need to agree on offsets.

### 3. Cells are rendered by recursion

A cell's source (`pipe_table_cell`) goes through the same inline rendering. Its `Invisible` ranges are dropped from the text and its face ranges are kept, mapped onto the shown chars. Widths, alignment from the delimiter row's `:` markers, padding and the header's `Bold` plus `session-block` face are computed on those shown strings.

Ragged rows are padded or truncated to the header's cell count, as GFM does. An escaped `\|` shows as `|`.

### 4. One line decoration: `Decoration::Table { edges, first }`

`edges` holds the char columns of the cell boundaries, which for ` a ` and `  b ` are 0, 3 and 7. `first` marks the header. It is added as a `Prop::Line` on each drawn line.

It does three jobs:
- `wrap_lines` treats it like `aligned`: one row, cut with `cut_spans` to the columns plus one, and no `\` marker.
- The sideways offset applies to the run of consecutive table lines around point's line, not just point's line, and the edges are shifted by it.
- The frontend paints its rules.

*Alternative:* `│` characters in the text. Rejected because at 15/21 the glyphs leave gaps between rows, and copying would include them. Separate `NoWrap` and `Rules` decorations were rejected too: nothing else needs either, so one decoration is enough.

### 5. Sideways scrolling with no new window state

`Window::hscroll(col, cols)` is reused as is. When point's line carries a `Table` decoration, the display builder walks up and down to the edges of that run and applies the offset it returns to every line in the run. Outside a table the behaviour is unchanged. Wrapping buffers then use `hscroll` for table runs only, and other lines keep offset 0.

### 6. Painting

For each row with a `Table` decoration, `element.rs` emits quads in `separator`'s foreground:
- a 1 px vertical line at `text_x + (edge - offset) * cell_width` over the full row height, so consecutive rows join;
- a 1 px line along the bottom from the first visible edge to the last;
- the same along the top when `first` is set.

Everything is clipped to the text area. This uses the same quad path as `Border`.

### 7. A link's destination rides on its label: `Prop::Url(String)`

`url_at_point` finds a link today by re-parsing point's line as markdown (`markdown::url_at`), which only works while the destination is still in the text. A drawn cell keeps only the label, so the cell renderer adds `Prop::Url(destination)` over the label's shown range, beside its `link` face. `url_at_point` checks the props at point for a `Url` first, and falls back to `url_at(line)` for text outside tables. From there the existing checks apply unchanged: an http or https URL opens, another scheme echoes `Not a web link: <url>`, and a destination with no scheme returns `None`, so file links try the label's word.

The display and frontend ignore `Url`. A buffer allows several props on one range, as with `Line` decorations beside faces.

*Alternative:* keep the destination in the cell as hidden text. Rejected because the edges and the sideways offset count chars, so hidden chars in a cell would misplace the rules.

## Risks / Trade-offs

- [Point inside a table jumps to its line's start when a widening column rewrites every row] → Only while that message is still streaming. `rewrite` keeps point on lines that didn't change.
- [Re-parsing and diffing the whole streamed block every frame] → Same order of work as today's whole-block re-parse. The streamed-markdown scenario in `crates/aiditor-core/benches/hot_paths.rs` gets a table.
- [Copying a table no longer gives markdown] → This was accepted deliberately, as Zed does. A later `agent-copy-markdown` can use `Stream`/`Message` sources if they're kept after the stream ends.
- [Wide characters misalign columns] → `shortcut:` comment at the width measurement. Upgrade it to `unicode-width` if CJK or emoji tables show up.

## Migration Plan

There's no data to migrate. Sessions replayed after the upgrade render tables the new way. `docs/editor.md`'s display-model paragraph ("the markup stays in the text") gets the table exception. The `m6-md-tables` task in `docs/roadmap.yaml` and its issue are rewritten to this approach, following the roadmap skill.
