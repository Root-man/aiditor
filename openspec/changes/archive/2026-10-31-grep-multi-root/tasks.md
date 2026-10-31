# Tasks

## 1. Core command

- [x] 1.1 Promote the current file's most specific root while preserving finder names and remaining project order; verify with harness tests for primary, additional, outside, scratch, nested and duplicate-name roots.
- [x] 1.2 Add prefix-triggered fuzzy single-root selection and suspend grep polling during selection; verify selection, cancellation, history and hit acceptance with harness tests.

## 2. Search service

- [x] 2.1 Traverse roots sequentially with parallel file workers and root-relative hit paths; verify root order, nested roots, ignore patterns and exact global caps with production service tests, updating superseded cross-root concurrency tests.
- [x] 2.2 Preserve streaming inside each root, cancellation and bounded workers; verify existing deadline/backpressure tests and a same-root stalled-file streaming test.

## 3. Integration and documentation

- [x] 3.1 Update docs/editor.md to describe root priority and prefix selection, replacing the superseded cross-root concurrency wording; verify consistency with the delta specification.
- [x] 3.2 Exercise root selection and result order through GPUI's test platform and production services; verify the test passes.
- [x] 3.3 Run formatting, workspace clippy/tests, CI structural checks, benchmark reporting tests and OpenSpec validation; record results for branch review.
