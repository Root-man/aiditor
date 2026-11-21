# Proposal

## Why

The inline diff (#505) draws removed lines in plain `diff-removed` and the `Staged` side's index text uncoloured, so a review reads worse than the file. #506 highlights the diff as the file's own buffer would.

## What Changes

- The new side keeps its buffer's highlighting; the read-only index buffer of a `Staged` diff takes the file's grammar.
- The old side is parsed with the file's grammar in a hidden buffer, so text over the size limit is parsed off the core thread as file buffers are; removed rows take their syntax faces over `diff-removed`, and are redrawn when that parse arrives and when the theme changes.
- Added lines keep their syntax faces beside the `diff-added` bar; a language without a grammar shows plain text in the diff faces.

## Capabilities

### New Capabilities

### Modified Capabilities
- `diff-view`: diff rows and side buffers are syntax highlighted.

## Impact

`aiditor-core`: `diff.rs` (old-side buffer, highlighted rows, redraws), `buffer.rs` (`FaceRef::Is`, giving a buffer a file's grammar), `settings.rs` (resolving it), `lib.rs` (redraw on `Parsed` and `SettingsLoaded`). `docs/editor.md`.
