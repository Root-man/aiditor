# Proposal

## Why

Large projects need an explicit way to avoid recursive root watches while retaining auto-revert for visited files. The launch HOME project already skips root watches through a special case; a persisted project setting makes this behavior configurable.

## What Changes

- Add boolean `watch`, defaulting to true in project files and false for an ad-hoc HOME project.
- Add `project-set-watch` under `C-x p w`, with true/false choices and the existing safe project editing behavior.
- Show unwatched project status in the mode line.
- Omit unwatched project roots from watcher configuration and ignore automatic index refresh for those projects, keeping explicit indexing and reload refresh available.
- Preserve parent-directory watches and auto-revert for visited files.

## Capabilities

### New Capabilities

### Modified Capabilities

- `projects`: Persist and edit the watch setting, with launch defaults and visible status.
- `file-watching`: Select project roots by watch setting instead of a HOME special case.
- `file-index`: Refresh unwatched projects only on demand.

## Impact

Core project model/TOML synchronization, project commands and bindings, watch selection and index event handling, display status, focused tests, and editor documentation. Existing watcher service and dependencies stay sufficient.
