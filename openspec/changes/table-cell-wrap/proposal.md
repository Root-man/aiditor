# Proposal

## Why

A drawn table in a session buffer is one rope line per row however long its cells are, so a wide table runs off the window and has to be scrolled sideways to read (#474).

## What Changes

- A new `table_width` setting (bundled as 100, a nonnegative integer) caps a drawn table's width in columns, rules and padding included.
- A table whose natural width passes the cap keeps the columns that fit their share at their natural width and shares the rest among the wide ones; their cells wrap at words onto more rope lines of the same row, and a word longer than its column is broken.
- Alignment applies to each line of a wrapped cell.
- Only a row's last line draws the row's bottom rule; column rules stay on every line, and the top rule stays on the header's first line.
- A table that fits draws as today. The cap doesn't depend on the window: a window narrower than the table still cuts it and scrolls sideways.

## Capabilities

### New Capabilities

### Modified Capabilities
- `session-buffers`: tables wrap their cells at `table_width`.
- `frontend-rendering`: a row's bottom rule is drawn only under its last line.
- `display-model`: a table line decoration says whether it ends its row.
- `settings`: the `table_width` setting.

## Impact

- `crates/aiditor-core/src/markdown.rs` (`draw`, `render`), `agents/render.rs` and its caller in `agents.rs` (pass the cap).
- `buffer::Decoration::Table` gains a field; `display.rs` and `crates/aiditor/src/element.rs` follow it.
- `settings.rs` and the bundled `settings.toml`.
- `docs/editor.md` (tables paragraph, settings).
