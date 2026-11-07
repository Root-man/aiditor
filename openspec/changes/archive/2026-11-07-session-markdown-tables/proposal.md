# Proposal

## Why

Agents answer in markdown, and session buffers render it by hiding markup in place: the buffer text stays the agent's markdown. That works for `**bold**` but not for GFM pipe tables, whose rendered layout (padded, aligned columns with rules) is nothing like their source, so today they show as raw pipes. The planned `m6-md-tables` approach would fake the layout with `Display` overlays and then patch motion, wrapping and column counting around them. Putting the drawn table into the buffer as real text, eww-style, makes motion, search and kill work with no special cases.

## What Changes

- Each streamed message block keeps its markdown source in the session. Rendering turns the source into the text to show plus its properties, and the block's buffer range is rewritten with that text.
- A GFM pipe table in an agent message becomes drawn text: cells padded to their column's width, aligned as its delimiter row says, inline markup in cells rendered and not counted in widths, the header bold on the `session-block` background. Column and row rules are painted in `separator`, not drawn with chars: a 1 px line between columns and under each row, spanning only the table's width.
- Links in cells work as they do elsewhere in a message. A markdown link's destination is kept on its label, since the destination is no longer in the text, and `RET` opens it.
- Everything outside tables keeps today's rendering, with the markup in the text and hidden.
- Table lines never wrap. They are cut at the window edge in every buffer, and when point is on a table, all of its lines scroll sideways together to keep point visible.
- **BREAKING**: search, copy and kill see a table's drawn text, as Zed's plain copy does. Copying a table gives its cells aligned with spaces, not its markdown. Everywhere else, markup still stays in the text.
- The `m6-md-tables` roadmap task is replaced by this approach.

Not in this change: tables in thoughts, tool-call bodies and the echoed prompt, which stay raw; rendering inline markup out of the text; a command to copy a message's markdown; wide characters in cells, which are measured in chars.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `session-buffers`: messages render tables as drawn text, and the "Markup stays in the text" requirement makes an exception for tables.
- `display-model`: a new `Table` line decoration, and table lines are cut instead of wrapped and scroll sideways together.
- `frontend-rendering`: the `Table` decoration's column and row rules are drawn.

## Impact

- `crates/aiditor-core/src/markdown.rs`: `render` returns the text to show along with its properties.
- `crates/aiditor-core/src/agents.rs` and `agents/render.rs`: `Stream` keeps the source. `output`, `render_stream` and `trim_message` work on the source, and message lengths come from the rendered text.
- `crates/aiditor-core/src/buffer.rs` (`Decoration::Table`, and `Prop::Url` for a link's destination, which `url_at_point` in `agents.rs` reads first), `display.rs` (no wrapping, sideways scrolling for a table) and `window.rs` (`hscroll`).
- `crates/aiditor/src/element.rs`: painting the table rules.
