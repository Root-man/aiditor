# Design

## Context

The snapshot renderer currently inserts a header and blank line. The frontend already owns echo/minibuffer layout and measures the remaining window area before sending Resize to the core. Core messages already clear on the next input and survive timers. Existing WorkspaceCurrent and ModeLineDim faces express frames 6d/13b.

## Goals / Non-Goals

**Goals:** Carry contextual action entries in FrameDisplay and let the frontend lay them out using the same font metrics as the echo line. Keep header removal correct for first-row offsets, properties and selection.

**Non-Goals:** No new commands, key bindings, live list refresh or #426 row-column fitting. No stored hint message or timer.

## Decisions

- One static `(key, action)` table in workspace_list supplies every hint; FrameDisplay exposes it only when the selected buffer owns a workspace-list snapshot, without a message/minibuffer. This preserves normal message lifetime and avoids logging hints to Messages. Existing which-key hints remain separate.
- Use the frontend's wrapping flex layout with one styled, non-wrapping child per action and three spaces between actions. Keys use WorkspaceCurrent foreground with bold, labels ModeLineDim. The normal echo row height applies to each row. This uses actual available width and naturally reduces FrameElement's allocated height, so Resize and cursor/terminal sizing use the reduced area.
- Insert separators before every row except the first. Row starts account for that separator, so removing the header introduces neither a blank first row nor off-by-one property ranges. Update header-dependent fixtures and explicit informational-row positions.

## Risks / Trade-offs

- Header removal shifts line offsets by two → retain exact tree fixtures and navigation/fold tests.
- Echo wrap changes height on resize → GPUI tests compare hint bounds and window bottoms across widths, messages, split selection and themes.
- At widths too small for a complete action, its text clips within that row; progressively narrower frames can need more than two rows. No truncation or horizontal scrolling is added.
- Native GUI screenshots and Linux execution are unavailable in this session → automate acceptance with the core harness and GPUI test platform and report those validation limits.

## Migration Plan

This change extends the approved local stack. After consolidated shipping approval, sync/archive workspace-list-project-rows, workspace-list-columns, workspace-list-folds and then workspace-list-hints in that order. Until then, those active deltas provide the current specification baseline; they are preserved unchanged. No runtime migration is required.
