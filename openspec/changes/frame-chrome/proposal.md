# Proposal

## Why

The frame still draws an 8px side gap, one-line-high mode lines and no divider, so it doesn't match the approved visual design (issue #353, frames 1, 2, 11 and 12). Everything else in the visual epic builds on this spacing.

## What Changes

- Windows run edge to edge: the frame's 8px side gap and 2px bottom gap go. Text sits 12px from a window's left and right edges where no gutter stands, and the gutter keeps its 10px + digits + 12px. A buffer's text starts 6px under the window's top.
- A 1px `divider` separates side-by-side windows. No other borders.
- Mode lines and the echo line are the line height plus 5px, rounded, 26px at 15px type. Their text and minibuffer, hint and debug rows sit 12px from the edge.
- An inactive window's mode line is in `mode-line-inactive` with a 1px rule on top.
- New `line_height` setting, a multiple of the font size, 1.0 to 3.0, bundled 1.4.
- New `workspace_font_size` setting, 6 to 72, bundled 13. The workspace modeline is its own line height plus 4px, rounded, and zooms with the frame.
- Ligatures are off everywhere, with no setting.
- The bundled `font_size` and theme stay as they are.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `frontend-rendering`: edge padding, divider, mode line and echo line heights, the inactive rule, the workspace modeline's size, ligatures.
- `display-model`: the configurable line height, rows left under the chrome, the divider in the layout.
- `settings`: `line_height` and `workspace_font_size`.
- `terminal-buffers`: the grid is sized to the inset text area.
- `text-scale`: the workspace modeline follows the zoom.
- `line-numbers`: wrapping leaves the right inset.

## Impact

- `aiditor-core`: `settings.rs`, `settings.toml`, `display.rs`, `frame.rs`, `window.rs`, `terminal.rs`, `minibuffer.rs`, `harness.rs` and tests that assume a one-line mode line.
- `aiditor`: `element.rs`, `editor.rs`.
- `docs/editor.md`: frontend, line numbers, and text scale text.
- Windows show slightly fewer rows and columns, and terminals resize to match.
