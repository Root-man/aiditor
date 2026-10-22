# Proposal

## Why

Large projects can exhaust watcher resources, particularly Linux's per-directory inotify watches. Issue #192 needs a filtered count before root watching begins.

## What Changes

- Add `max_watched_files = 10000` to bundled settings and an optional project override.
- Count indexed files and kept directories asynchronously after ignores; refuse root watches when either exceeds the limit.
- Report the counts and setting in echo and `*Messages*`, with `[unwatched]` and the same visited-file fallback and manual indexing as `watch = false`.
- Reevaluate on index results and settings/project reloads; zero disables root watching because roots count as directories.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `file-watching`: gate root watches on filtered counts and retain visited-file fallback.
- `settings`: global limit and project precedence.

## Impact

Settings/project models and constructors; index service, messages and harness; core watch policy and mode lines; docs and tests. No new dependency or command. No file content changes or migration.
