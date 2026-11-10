# Design

## Context

`markdown::draw` turns a pipe table into one rope line per row and gives each line a `Decoration::Table { edges, first }`; the frontend's `table_rules` draws column rules, the bottom rule on every line and the top rule on the first. `markdown::render` takes `tables: bool`; only `render_message` (streamed agent messages) and its debug check pass `true`.

## Goals / Non-Goals

**Goals:** cap a table's width at a setting, wrapping the wide columns' cells, with rules that still read as one row.

**Non-Goals:** depending on the window's width (the rope is shared by every window), re-rendering transcript tables when the setting is reloaded, measuring display width of wide glyphs (widths stay chars, as today).

## Decisions

- **The cap reaches `render` in place of the bool.** `tables: Option<usize>`: `None` keeps tables as text, `Some(cap)` draws them at that cap. No new parameter, and the callers that pass `false` pass `None`.
- **Column widths.** Room is `cap - 2 × columns` (each cell's two spaces). Repeatedly: the share is the room left over the unfixed columns; every unfixed column whose natural width fits it is fixed at its natural width and its width taken from the room; stop when none fits. The rest get the share, the first ones one more for the remainder, at least 1 each. This is the "fixed columns that fit their share" the issue asks for, and keeps narrow key columns intact. A cap too small for one char per column gives a table wider than the cap.
- **Wrapping.** Greedy per cell over its shown chars: a line ends at the last space within the width, else the word is cut at the width; spaces at the break are dropped. Each cell line is a char range of the cell, so faces and links keep their per-char lookup.
- **Decoration.** `Decoration::Table` gains `last: bool`, true on a row's last line; `table_rules` draws the bottom rule only then. `first` stays the table's first line. The header's `session-block` bold base covers all its lines.
- **Setting.** `table_width: usize` in `Settings`, bundled as 100 in `settings.toml`; serde rejects bad values through the existing error flow, as `paste_fold_lines`. `render_stream` reads it from `Core::settings`.

## Risks / Trade-offs

- [A reload doesn't re-wrap tables already drawn] → only messages rendered afterwards (including the streaming one) follow; noted in the spec ("tables drawn afterwards").
- [Streaming tables can re-flow columns as rows arrive] → same as today's widening; the tail render already rewrites the table's block.
