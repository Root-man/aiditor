# Design

## Context

The core already owns project TOML edits, watch-set generation, file-event index refresh and frame rendering. The service watches visited files through parent directories when no root covers them. See proposal.md for motivation.

## Goals / Non-Goals

Goals: carry one project boolean consistently through persistence, watch selection, commands and visible frame status. Non-goals: watcher limits (#192), new watcher APIs, continuous finder updates, manual GUI verification or PR creation.

## Decisions

- `watch` defaults true when omitted. Serialize true sparsely to keep existing project files unchanged; TOML synchronization preserves explicit booleans and comments. Ad-hoc construction defaults false only when its root is HOME, replacing the old watch-set special case. Saving that project retains false.
- Add `project-set-watch` on `C-x p w`, completing true/false with the current value selected. Reuse the existing checked edit path for saved and ad-hoc projects and reject any other text. A toggle-only command would not allow an explicit target value.
- Add ` [unwatched]` to ordinary frame window mode lines for the current workspace's unwatched project. This reflects launch, reload and command changes without repeated Messages logs. The test-only single-buffer display helper remains focused on buffer rendering.
- Filter watch roots by the boolean. Independently filter event-driven index refresh, including rescan, so visited-file reports or another project's shared roots cannot refresh an unwatched project. Explicit project-index, initial finder indexing and user-requested settings reload remain available.

## Risks / Trade-offs

- An unwatched index becomes stale after filesystem changes -> `C-x p i` explicitly refreshes it; file buffers continue auto-reverting.
- HOME can be enabled deliberately and may be large -> the default remains false, and limits are tracked separately in #192.
