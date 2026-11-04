# Tasks

## 1. Project inventory and selection

- [x] 1.1 Introduce typed project/workspace targets and list every project with roots and empty-project labels; verify ordering, identity and escaped labels in harness tests.
- [x] 1.2 Navigate selectable rows and activate projects through project-switch; verify every key family, end clamping, buffer-row refusal, stale targets, MRU and first-workspace behavior in harness tests.

## 2. Mode line and documentation

- [x] 2.1 Render snapshot project/workspace totals and selectable position; verify frame output including informational lines and refresh.
- [x] 2.2 Update docs/editor.md to match the implemented contract and validate the pending workspaces delta with openspec validate --strict.

## 3. Integration checks

- [x] 3.1 Run formatting, workspace clippy/tests and the remaining CI checks; assess accessible native checks and report unavailable coverage.
