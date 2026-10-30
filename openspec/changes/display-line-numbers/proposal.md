# Proposal

## Why

File buffers have no line numbers, though the approved design draws a gutter in every file window (frames 1, 2 and 11). Grep hits, compiler errors and `M-g g` all speak in line numbers, so reading code without them means looking at the mode line.

## What Changes

- File buffers, `*scratch*` and `*Messages*` show a gutter of line numbers: 10px, the digits of the buffer's line count, 12px.
- Point's line number is in `line-number-current`, every other in `line-number`; a soft-wrapped line's continuation rows show `↪` in `line-number`.
- A new `line_numbers` setting takes `off`, `absolute` or `relative`; the bundled value is `absolute`, as the design draws.
- `display-line-numbers-mode` toggles the gutter for the current such buffer.
- Text wraps and scrolls sideways in the window's width less the gutter.

## Capabilities

### New Capabilities

- `line-numbers`: the gutter of file buffers, `*scratch*` and `*Messages*`, its numbers and width, the `line_numbers` setting and the toggle.

### Modified Capabilities

None.

## Impact

`aiditor-core`: `settings.rs`, `settings.toml`, `buffer.rs`, `display.rs`, `frame.rs`, `builtins.rs`. `aiditor`: `element.rs`. `docs/editor.md`. No new dependency.
