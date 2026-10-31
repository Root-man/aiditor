# Design

## Context

`Core::frame` currently fits one project/workspace pair; the frontend draws three children. Workspaces already live in creation order, buffers carry their workspace identity, and sessions track prompt, permission and exit state. The core must remain independent of system time. GPUI provides cancellable tasks and test-clock timers; the workspace already uses jiff for local timestamps on both target platforms.

## Goals / Non-Goals

**Goals:** Keep fitting and activity aggregation deterministic in the core, use the existing span/face model in the frontend, and wake the quiet UI only at minute boundaries.

**Non-Goals:** Clickable workspace tabs, persistence, a timezone setting, new theme faces, or changes to the window title.

## Decisions

- Replace the frame's two fitted strings with workspace spans, composed over `workspace-bar`. Share a session activity accessor with its mode line so the two agree; aggregate by owning buffer's workspace, never the session's saved name. Priority is waiting, failed, working.
- Reserve seven workspace-font columns for clock and gap. Fit full entries first; otherwise expand a contiguous range around the current entry, nearest first, previous on a tie. Recalculate `+N` and hidden activity as the range changes. If only the current entry fits, shorten its names with the existing sharing rule. At extreme widths drop overflow before current names, then project/separator and dot as necessary. This preserves task identity without horizontal scrolling or clipping neighbours.
- Keep the existing fixed monospace-column fitting convention, scaled for workspace font size. Render spans with GPUI styled text so face bold/italic settings survive.
- Read the local clock in a small service using the already installed jiff dependency, returning `HH:MM` and duration to the next minute. A retained frontend task sleeps via GPUI, reads wall time again, updates the clock and notifies. It cancels when the editor is dropped. Keep wall time outside core messages and snapshots.
- Defaults: local 24-hour time, idle/starting sessions without dots, exited sessions treated as failed as in their mode lines, and a single trailing overflow indicator for hidden entries on either side.

## Risks / Trade-offs

- Fixed-column fitting inherits the existing limitation for wide/fallback-font glyphs; frontend containment keeps them clear of the clock. Actual glyph-width fitting is outside this issue.
- Wall-clock adjustments are observed at the next scheduled wakeup, at most one minute later; recalculating each wakeup avoids cumulative drift.
- The published canvas was inaccessible through the web tool; issue #360 and `docs/editor.md` provide the concrete dimensions, faces and behavior used here.
