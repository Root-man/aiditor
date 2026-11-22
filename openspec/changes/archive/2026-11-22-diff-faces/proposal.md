# Proposal

## Why

The diff view works (#504, #505, #506), but it isn't drawn as frame 15 shows it: added lines carry a bar instead of a background, removed rows are red text labelled with old line numbers, there is no sign column, the list has no fold mark, full-length paths, and no `hl-line` on the shown file, and the echo line's hints show only in the list. #508 draws it as the frame does, in both Gruvbox themes.

## What Changes

- Added lines take `diff-added`'s background under their syntax faces, and a `+` in `diff-added` in a sign column between the line numbers and the text. Removed rows take `diff-removed`'s background, a `-` there and no line number, their text in the old side's syntax faces alone.
- The list: `▾`/`▸` before each git root, which `TAB` folds; the directory part of a path in the dim face, and paths cut from the left with `…` to fit the window; the shown file's row in `hl-line` whether or not the list is selected.
- The mode line puts `change <i>/<n>` after the branch, before point's position, as the frame does.
- The echo line shows the view's key hints in the diff window too, as `C-c n/p change` and so on.
- The bundled theme gives `diff-added` and `diff-removed` backgrounds, so the default theme draws the view too.

## Capabilities

### New Capabilities

### Modified Capabilities
- `diff-view`: the list's fold, faces and fitting; the diff window's backgrounds, sign column, mode line order and hints.

## Impact

`aiditor-core`: `diff.rs` (rows, list rendering, folding, hints), `window.rs` (`DiffRow::sign`), `display.rs` (`Line::sign`, the gutter's signs, list fitting), `frame.rs` (backgrounds, sign column, hl-line in the list, mode line order, hints), `builtins.rs` and `keymap.toml` (`diff-toggle` on `TAB`), `settings.toml` (backgrounds). `aiditor`: `element.rs` draws the sign column. `docs/editor.md`.
