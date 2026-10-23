# Tasks

## 1. Configuration reload feedback

- [x] 1.1 Add completion feedback to the accepted-bundle path in `configuration.rs`, isolating its diagnostics from earlier echo messages and retaining component fallback; verify core tests for clean reloads via keys and M-x, read/parse/binding/project diagnostics, and an unrelated message during a pending reload.
- [x] 1.2 Extend controlled reload tests to assert one logged confirmation for the latest accepted bundle, including duplicates, superseded results and a project-revision retry; verify startup remains on its existing message path and completion uses the normal key/click-cleared echo lifetime.

## 2. Active project default

- [x] 2.1 Preselect the active workspace's project in workspace-new's first prompt, carry the accepted candidate's project ID through completion into `Args.project`, and retain literal-input lookup and name validation; verify default acceptance when the active project is not first, selection of another project, and two equal-named projects with different roots.
- [x] 2.2 Cover the prompt chain and identity lifetime with key-script tests for blank and explicit workspace names, cancellation of either prompt, unknown projects, duplicate workspace names, and a project reload/removal between project selection and name acceptance; verify creation uses the selected live identity or reports an error without creating a workspace.

## 3. Temporary workspace tree

- [x] 3.1 Replace `list_workspaces`' echo output with a read-only, fileless, unmodified workspace-local list buffer selected in the current window; render groups by project identity, workspace rows and sorted listed owned buffers with a current-workspace marker, and attach workspace IDs to row positions. Verify the hierarchy, equal project names, hidden/global/list-buffer filtering and escaped multiline labels with focused core tests.
- [x] 3.2 Add mode-local next/previous commands and bindings for n/p, C-n/C-p and arrows, selecting the current workspace initially; verify movement skips informational rows, clamps at both ends and brings selected rows into view in a long list.
- [x] 3.3 Add RET activation and q/C-g dismissal using existing workspace switching and normal buffer-kill replacement; verify another workspace's saved layout and point, choosing the current workspace, an informational row, and a stale workspace ID.
- [x] 3.4 Reuse and refresh the invoking workspace's list on each invocation and remove row metadata on ordinary buffer kill and workspace teardown; verify refresh after buffer/workspace changes, kill/reopen, per-workspace ownership, and switching away from an open list through existing workspace commands.
- [x] 3.5 Replace the old echo-string list test and cover both C-x t l and M-x workspace-list, read-only edit rejection, retained user text and split structure, and ordinary key behavior outside workspace-list-mode; verify the focused workspace and keymap tests pass.

## 4. Documentation and integration

- [x] 4.1 Update `docs/editor.md` configuration and workspace command descriptions with completion feedback, current-project default, tree layout, workspace-only activation, snapshot refresh and dismissal semantics; verify descriptions agree with both delta specs and remove the obsolete echo-only description.
- [x] 4.2 Run `cargo fmt --all -- --check`, `cargo test -p aiditor-core` and `cargo test -p aiditor-services` to verify formatting and integration with existing project reload, command, minibuffer and buffer lifecycle behavior; resolve regressions introduced by this change.
- [x] 4.3 Smoke-test on macOS and Linux: successful/failed reload feedback, default/alternate project creation, a multi-project tree long enough to scroll, workspace-only navigation and activation, and dismissal. Record platform results and any unavailable platform explicitly; this check does not close the separate M1 gate in #85.

## 5. Fileless buffer mode lines

- [x] 5.1 Gate `[unwatched]` on the displayed buffer's file association in `Core::frame`; update frame, watch and index tests to cover mixed file/scratch splits, fileless temporary buffers, and retained file-backed eligibility labels.
- [x] 5.2 Update `docs/editor.md` to restrict the label to file-backed windows; run core tests, formatting, lint and strict OpenSpec validation for this follow-up.

## Verification record (2026-10-23)

Core tests: 399 passed, one existing ignored test. Services: 79 passed across unit, headless and integration targets, run outside the sandbox with piped output for native watcher and file-size-limit tests. Formatting, all-target Clippy with warnings denied, diff whitespace checks and strict OpenSpec validation passed. Initial service failures were resolved by the test execution environment, without service code changes.

Task 4.3 accepted by the user on 2026-10-23: “Manual UI test is ok.” Platform-specific results were not supplied; this records user acceptance, not agent-executed macOS/Linux testing. The separate M1 gate in #85 remains outside this change.

Follow-up: the user requested committing the existing work and applying the fileless-buffer label correction. The existing work was committed as `0e8aea9`; tasks 5.1–5.2 are implemented separately. Core tests now pass 400 tests with one existing ignored test. Formatting, all-target Clippy with warnings denied, diff whitespace checks and strict OpenSpec validation pass. Services tests were not rerun for this display-only follow-up. The manual UI task was subsequently accepted by the user as recorded above.
