## Why

Issue #57 is the open question of nested or overlapping roots, within one project or across projects, and of opening a single folder without a project. #58 (project commands) is blocked on it, and the loader built in #56 accepts a project whose roots overlap, so find-file, grep and the watchers would see the same files twice.

## What Changes

- Record the decision in `docs/editor.md` under Projects:
  - within one project, a root that equals or sits inside another root (the primary included) is rejected at load;
  - across projects, shared or overlapping roots are allowed: each workspace is bound to one project, so a shared root is never ambiguous;
  - opening a single folder (on the command line or with an open-folder command) makes an ad-hoc project held in memory, with the folder as primary and its name as the project name, and a workspace for it; nothing is written until a save-project command writes it to `projects/`.
- Remove both questions from Open questions and the Projects bullet that points at them.
- Build the first rule now: the project loader rejects overlapping roots with `project <file>: <path> is inside <path>`.
- `docs/roadmap.yaml` and GitHub: #58 gets save-project for an ad-hoc project, #64 gets open-folder.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `projects`: overlapping roots within one project are a load error.

## Impact

- `crates/aiditor-core/src/project.rs` (`Project::parse` and its tests), `docs/editor.md`, `docs/roadmap.yaml`, issues #58 and #64.
