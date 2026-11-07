# Tasks

## 1. Render returns shown text

- [x] 1.1 Change `markdown::render` to take a `tables` flag and return `(String, props)` with offsets into the returned text; with `tables` off the text is the source and every existing `markdown.rs` test passes unchanged; update the callers in `agents/render.rs` (stream, echo, tool body) to the new signature and verify `cargo test -p aiditor-core` passes
- [x] 1.2 Draw `pipe_table` nodes when `tables` is on (design decisions 2 and 3): cells rendered inline with markup dropped, widths in shown chars, alignment from the delimiter row, ragged rows padded or cut to the header, header in `Bold` + `session-block`, link faces found on the shown text, a `Prop::Line(Decoration::Table { edges, first })` per line, and a `shortcut:` comment on char-counted widths; verify with `markdown.rs` tests for widths, each alignment, markup in a cell (`**hot** path` → `hot path`, width 8), a ragged table, edges `0, 3, 7` for `| a | b |`, and text outside the table unchanged with its props shifted

## 2. Streamed messages keep their source

- [x] 2.1 Add `source` to `Stream`; `output` appends each message chunk to it as well as inserting it; `render_stream` renders the source with tables on, `rewrite`s the block's range with the text, adds the props and corrects `Message.len`; `trim_message` trims the source too; verify with `agents.rs` tests for the session-buffers scenarios "Columns and alignment", "A table still streaming", "Copy yields a drawn table" and "Search sees a drawn table", and that the existing streaming, message-bar and `C-c C-p`/`C-c C-n` tests still pass
- [x] 2.2 Add `Prop::Url(String)` in `buffer.rs` (ignored by display and frontend), put it over each markdown link's label in a drawn cell, and make `url_at_point` read it before falling back to `markdown::url_at`; verify with `agents.rs` tests for the scenarios "A link in a cell" and "A link in a cell to another scheme", and that the existing web-link and file-link tests pass
- [x] 2.3 Check that a replayed session (`session/load`) draws its tables and that a thought with a table stays raw; verify with an `agents.rs` test over a replayed transcript holding a table
- [x] 2.4 Add a table to the streamed-markdown scenario in `crates/aiditor-core/benches/hot_paths.rs` and verify `cargo bench -p aiditor-core --bench hot_paths` runs and reports it

## 3. Display: cut and scroll table lines

- [x] 3.1 Add `Decoration::Table { edges, first }` in `buffer.rs`, carried to display lines like the other decorations, with its edges shifted by the line's horizontal offset; verify with the display-model scenario "A table row"
- [x] 3.2 In `display.rs`, keep table lines to one row, cut at the window's columns plus one with no `\` marker, in wrapping and truncating buffers alike; verify with the scenario "A wide table in a narrow window"
- [x] 3.3 Apply `Window::hscroll` to the run of consecutive table lines holding point, in wrapping buffers too, with other lines at offset 0; verify with the scenarios "End of a table row" and "Leaving the table", and that the existing horizontal-scroll and wrap tests pass

## 4. Frontend: paint table rules

- [x] 4.1 In `crates/aiditor/src/element.rs`, paint the `Table` decoration in `separator`'s foreground: vertical 1 px lines at each visible edge across the full row height, a bottom line between the first and last visible edges, and a top line on `first`, all clipped to the text area; verify with a frontend test if the element has paint tests, else by running aiditor on Linux with a fake-agent transcript holding a wide table and checking the frontend-rendering scenarios "A two-row table" and "A table cut by the window" in a screenshot

## 5. Docs and roadmap

- [x] 5.1 Update `docs/editor.md`: the session display-model paragraph and the markdown note ("Lists, quotes, tables … are shown as text") get the drawn-table exception, the `Table` decoration, the cut-not-wrapped rule and the shared sideways offset; verify by rereading both passages against the specs
- [x] 5.2 Following `.claude/skills/roadmap/SKILL.md`, rewrite `m6-md-tables` in `docs/roadmap.yaml` and its GitHub issue to this approach and acceptance; verify that `scripts/roadmap.py` validation passes and the issue body matches

## 6. Integration

- [x] 6.1 Run `cargo test --workspace` and `cargo clippy --workspace` on Linux, and confirm that a live Claude session in aiditor draws a markdown table with aligned columns and rules and that `M-w` over it yields its drawn text

## Workflow follow-up

- Run `/opsx:verify`, then `/opsx:archive` to sync the three spec deltas into `openspec/specs`.
