# Proposal

## Why

A project as large as `$HOME`, or a repo with big untracked trees that `.gitignore` doesn't cover, indexes, greps and watches far more than matters. Issue #190 adds Projectile's `.projectile` for aiditor: ignore patterns kept in the project file, so a project can be narrowed, down to an allow-list of the directories that matter, without touching the repos' own ignore files.

## What Changes

- The project file gets an optional top-level `ignore` key: a list of gitignore-syntax patterns, applied to every root of the project, each matched relative to its root.
- The file index, project grep and the watcher leave out what the patterns match, on top of what `.gitignore`, `.ignore` and git's global excludes already leave out. A pattern can't bring back a file those ignore.
- Negated patterns make an allow-list: the last pattern matching a path or a directory above it decides, so `*` then `!notes/` keeps only `notes/` and everything under it.
- Each pattern is checked when the project file loads (launch, `C-x C-r`, and every project write); an invalid one rejects the file with `project <file>: <reason>` like any other error.
- A `.ignore` file in a root keeps being honoured (already the case).

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `projects`: the file format gains `ignore`; an invalid pattern rejects the file.
- `file-index`: the walk also leaves out what the project's patterns match.
- `project-grep`: searches skip what the project's patterns match (they cover the index's files).
- `file-watching`: changes the project's patterns leave out are not reported, and on Linux their directories get no watch.

## Impact

- `aiditor-core`: `Project` gains `ignore`; `FileOp::Index` and `GrepSearch` carry the project's patterns; `WatchSet.roots` carries each root's patterns.
- `aiditor-services`: the shared walker (`files::walker`) applies the patterns; `projects::check` compiles them; the watcher keeps each root's patterns.
- No new dependency: the `ignore` crate already in `aiditor-services` compiles the patterns.
- `docs/editor.md`: the Projects section records the decisions.
