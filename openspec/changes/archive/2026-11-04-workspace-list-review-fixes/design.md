# Design

## Context

The pending five changes retain structured entries, rich buffer text and per-window fitted spans. Buffer eligibility is currently name-based, locations are strings, and rebuilding text resets all window point markers. Those structures can carry the required identity and location faces without a new subsystem.

## Goals / Non-Goals

Refine the retained snapshot while preserving split structure, current/selected decoration, fitting and navigation. Full-frame presentation remains outside this review fix.

## Decisions

- Filter owned inventory once by file/terminal presence or registered session buffer identity. Counts derive from that same vector; zero eligible buffers means no workspace fold marker.
- Store WHERE as the existing display Segment (text plus FaceName). Session model_effort supplies its faces directly; other locations use annotation. Escape location values per segment, and preserve faces when the existing leading-shortening rule cuts a location.
- Add buffer identity to entries, leaving the selectable Row enum as project/workspace only. Before rebuilding, capture each sharing window's row identity, column and ancestor chain. Restore by exact identity, then visible owner workspace/project, then current workspace/project; fallback selects the name. Existing window scroll stays, with normal redisplay moving only enough to show point. This avoids retaining stale numeric offsets or resetting sibling point markers.
- Represent separator rows in the shared visible traversal between adjacent project entries only. Render one ordinary space carrying Prop::Line(Decoration::HRule(Separator)); no dash text or frontend drawing mechanism.
- Make the shared Buffer::wrap decision return no wrapping for an actual workspace-list snapshot before row layout, scrolling or visual motion. The rich snapshot text may be wider than the window; wrapping it before fitted spans replace it would duplicate each logical row for every wrapped fragment. The truncation toggle remains ordinary buffer state but cannot make fitted list rows wrap. File and session wrapping keep their existing behavior.
- Add g to the workspace-list keymap and single hint table. Test its behavior through actual key dispatch.

## Risks / Trade-offs

A removed target cannot retain its old row → use documented stable ancestor/current fallback. Buffer IDs make informational rows stable across name/sort changes. Character-based fitting retains the existing wide-Unicode limitation. Native screenshots and Linux execution remain unavailable under the authorized automated-only scope.

## Migration Plan

Archive this change after project-rows, columns, folds, hints and narrow, in that order, so its MODIFIED requirements refine the resulting complete contract. No persisted data migration.
