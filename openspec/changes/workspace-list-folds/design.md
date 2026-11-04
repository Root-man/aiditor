# Design

## Context

See proposal.md. #422/#423 provide stable row identities, visible row positions, shared state semantics and three columns. Approved frames 13–13d define folding; frame 6 supplies the markers.

## Goals / Non-Goals

Keep one snapshot per list buffer and derive visible text/selection metadata from it. No filesystem persistence, live update, echo-hint redesign or narrow-column work.

## Decisions

- Retain a small hierarchy of rendered row values in list-buffer presentation state. A row owns its children and fold bit; rerender this snapshot on TAB. Rebuilding from Core on TAB would violate snapshot semantics.
- Match prior fold bits by project/workspace identity on explicit refresh, with open projects and only the current workspace open by default. If refresh retains a folded current project, select that visible ancestor instead of overriding its fold.
- Keep the existing visible selectable-row index for navigation/highlight/activation/position; retain full inventory totals separately in the snapshot.
- Reuse workspace_activity and its ordered Activity maximum for folded-project summaries. Draw fold markers with session-thought independently of current-workspace styling.
- TAB on informational and childless rows is a no-op. Empty projects have no trailing blank; only open projects with children get the separator, following frame 13b.

## Risks / Trade-offs

- Retained row values duplicate formatted text → bounded to the list's existing snapshot; destroyed on dismissal.
- Rerendering shifts other windows' point markers → verify shared-buffer display and selected point; do not introduce new window-selection behavior.
- Native CUA is disabled and Linux is unavailable → automate core display and GPUI input acceptance, report those limits without requesting manual evidence.

## Migration Plan

The user-authorized local stack keeps its predecessor changes unarchived until shipping approval. After that approval, sync/archive `workspace-list-project-rows`, then `workspace-list-columns`, then `workspace-list-folds`, and rerun strict validation. The first two introduce requirements this delta modifies; the current archive-refusal info is an expected prerequisite, not an implementation error. No runtime migration is required.
