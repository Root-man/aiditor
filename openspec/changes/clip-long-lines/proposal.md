# Proposal

## Why

Every shown line goes into the display model whole. `display::build` walks each char of it (a rope lookup, a fold search and a face scan over every property), and the frontend shapes the whole line with GPUI's `shape_line`, only to clip it with the window's content mask. That happens on every redisplay, which is every key. A minified 2 MB JS or JSON line or a long log line makes typing crawl. The display-model spec already says lines are "truncated at the window edge". Today only the frontend's clip makes it look that way, while the model still carries every char.

## What Changes

- The display model cuts each line to the window's columns: point's line from the horizontal scroll offset, every other line from column 0. The core stops walking a line once it is cut, and the frontend shapes only what is shown.
- A line holds at most the window's columns plus one char, so a partly visible last column still draws as it does today.
- A display with no width (0 columns before the first resize, or the test-only `Core::display`) cuts nothing, as it scrolls nothing today.
- Line decorations and folds that join lines behave as before, even when they sit in the cut-off tail.
- The cut moves into `display::build`. The trim of point's line in `window_display` goes away.

## Capabilities

### New Capabilities

### Modified Capabilities
- `display-model`: "Point's line scrolls sideways to keep point visible" now says the display model holds only the columns shown, not whole lines. Its "Back to a column that fits" scenario said the 100-char line is "shown whole", which a 40-column window can no longer do. It gains a scenario for a line far wider than the window.

`skip_specs` is not used. The display model is the contract between core and frontend, and what its lines hold changes in a way a test can see: the existing harness test `point_s_line_scrolls_sideways_to_keep_point_visible` asserts whole 100-char lines in a 40-column window, so it has to change.

## Impact

- `aiditor-core`: `display.rs` (`build` takes the shown columns and cuts lines; the `shortcut:` comment above it is updated), `lib.rs` (`window_display` settles the offset before `build` and drops its post-cut), and the harness tests for horizontal scroll.
- `aiditor` frontend: no code changes. It shapes shorter strings, and clicks still map because they already add `window.left()` on point's line.
- `docs/editor.md` Long lines bullet: one clause saying the cut happens in the display model.
- Out of scope: soft wrap (#112), display-width columns for tabs and wide chars, a truncation indicator (Emacs's `$`), and scrolling by display lines through folds.
