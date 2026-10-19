## Why

Issue #59 is the open question of what "switch project" does now that every workspace is bound to one project. The workspace commands (#64) are blocked on it.

## What Changes

- Record the decision in `docs/editor.md` under Projects: switch project prompts for a project with completion, then jumps to that project's most recently shown workspace, or creates a workspace for it and shows it when it has none. It never asks a second question; a further workspace on the same project comes from New workspace.
- Remove the question from Open questions and the Projects bullet that points at it.
- `docs/roadmap.yaml` and GitHub: the workspace commands task (#64) gets switch project in its body and acceptance criteria.

Not built here: the command itself lands with the workspace commands, since workspaces don't exist yet.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

None. Decision only; no behavior changes until #64 builds the command (`skip_specs: true`).

## Impact

- `docs/editor.md`, `docs/roadmap.yaml`, issue #64.
