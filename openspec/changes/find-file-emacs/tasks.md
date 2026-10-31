# Tasks

## 1. Listing metadata

- [x] 1.1 Carry annotated directory entries from the file worker through listing replies and harness fixtures; verify service tests for mode bits, symlinks, decimal sizes, ages and listing errors.

## 2. Completion behavior and rendering

- [x] 2.1 Add empty-query find-file selection and count behavior, retaining other prompts; verify core tests for five-entry navigation, typed RET, TAB, previews and directory transitions.
- [x] 2.2 Render directory names and right-aligned dimmed metadata with name clipping; verify GPUI tests for faces, highlights, selection and wide/narrow column layout.

## 3. Integration

- [x] 3.1 Document the new find-file behavior and run formatting, clippy, workspace tests, architecture checks, Python reporting tests and OpenSpec validation.

## 4. Project finder review changes

- [x] 4.1 Extend unselected empty-query behavior to project-find-file and update affected preview/navigation tests and check benchmark drivers; verify pending-index, TAB, typed acceptance and clearing behavior.
- [x] 4.2 Fetch and cache metadata for visible project matches through the captured root mapping; verify bounded requests, missing files, duplicate roots, reloads, stale sessions and name-only matching with core/service tests.
- [x] 4.3 Verify project finder metadata layout with GPUI, update docs, and rerun all CI checks.
