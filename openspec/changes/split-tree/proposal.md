# Proposal

## Why

The frame has exactly one window, so a buffer can only be seen in one place at a time. Issue #35 adds Emacs's window splits: a split tree with ratios and the commands `C-x 2`, `C-x 3`, `C-x o`, `C-x 0` and `C-x 1`, each window keeping its own point and scroll position.

## What Changes

- The frame holds a binary split tree of windows; each split is stacked (one above the other) or side by side, with a ratio that is 0.5 when made.
- New commands `split-window-below` (`C-x 2`), `split-window-right` (`C-x 3`), `other-window` (`C-x o`, takes a count), `delete-window` (`C-x 0`) and `delete-other-windows` (`C-x 1`).
- Every editing and motion command keeps acting on the selected window.
- The core builds a frame display: each window's rectangle in pixels for a given frame size, whether it is selected, and its window display.
- The skeleton frontend keeps drawing only the selected window; drawing the laid-out windows is #11's job.

## Capabilities

### New Capabilities

None.

### Modified Capabilities
- `windows`: the frame has a split tree of windows instead of one, plus the split, select and delete commands.
- `display-model`: a frame display lays the windows out by the split tree.

## Impact

- `crates/aiditor-core`: `window.rs` (window ids, the split tree and its layout, dropping a window's markers), `lib.rs` (the other windows, the five commands, `Core::frame`), `display.rs` (the frame display types), `keymap.toml`, harness tests and display snapshots.
- `crates/aiditor/src/main.rs`: unchanged apart from the new bindings reaching the core through the existing key path.
- `docs/editor.md`: notes on what was built.
- No new dependencies.
