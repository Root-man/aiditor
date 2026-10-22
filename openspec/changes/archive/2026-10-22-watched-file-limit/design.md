# Design

## Context

The core derives the whole WatchSet each message. The asynchronous file index already applies the watcher filters; its results can gate watches without blocking the UI.

## Goals / Non-Goals

Reuse indexing, existing watch=false behavior and visited-file fallback. No new commands, persistence of counts or OS-limit probing.

## Decisions

- Default 10,000; optional project usize override; zero disables roots. Both kept files and directories must fit, so Linux empty trees cannot bypass the guard. Count directories on every platform for portable configuration.
- Queue startup recovery through the service channel (even if empty) so preflight effects run after the UI and runtime exist; opening the finder while preflight runs promotes that index to fill its candidates on completion.
- Add kept-directory counts to index replies. Reuse file lists for file counts. Active watch=true projects start a quiet index if absent; roots wait for a successful answer. Incomplete counts fail closed.
- Centralize effective eligibility for roots, automatic index refresh and mode line. Keep stored watch=true unchanged so raising the limit restores watches.
- Remember current refusal counts/limit per active project to avoid repeated echo/log messages. Reload/reindex reevaluates; project edits invalidate the preflight.

## Risks / Trade-offs

- Startup waits for an index → run on the existing blocking pool; visited files remain covered.
- A refused project cannot observe shrinkage → manual project-index or reload reevaluates.
- Counts are snapshots, not a process-wide inotify quota → new directory events can transiently exceed the threshold before reindex finishes; no OS quota guarantees are claimed.
- Index walks skip unreadable descendant entries as before; unreadable roots refuse watching.
